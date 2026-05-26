//! Handshake: SessionInfoRequest → SessionInfo, per Tesla domain.

use std::time::Duration;

use anyhow::Context;
use prost::Message;
use rand::RngCore;
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::gatt::Connection;
use crate::keys::KeyPair;
use crate::proto::signatures::{SessionInfo, SessionInfoStatus};
use crate::proto::universal_message::{
    Destination, Domain, RoutableMessage, SessionInfoRequest, destination,
    routable_message,
};

/// Distinct session-handshake error so callers (notably the API
/// crate's pair handler and the UI status broadcasts) can render a
/// specific message for "your key isn't paired on the car" instead of
/// the generic "decode failed" path. Mirrors tesla-control's
/// `protocol.ErrKeyNotPaired` (returned by `protocol.GetError` when the
/// car's SessionInfo response status is
/// `SESSION_INFO_STATUS_KEY_NOT_ON_WHITELIST`).
#[derive(Debug, Error)]
pub enum SessionError {
    /// Tesla returned a SessionInfo whose `status` field explicitly
    /// says our public key isn't on the car's whitelist. The user
    /// needs to re-run the pair flow and tap the card on the
    /// center-console NFC reader — this is NOT a transport bug or a
    /// retryable error.
    #[error(
        "key not paired with car (SESSION_INFO_STATUS_KEY_NOT_ON_WHITELIST) — \
         re-pair from the SentryUSB UI and tap your card on the center console"
    )]
    KeyNotPaired,
    /// Any other handshake failure (decode error, transport error,
    /// unexpected payload shape). Carries the underlying anyhow chain
    /// so existing call sites that just bail/propagate don't lose
    /// diagnostic detail.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Result of one SessionInfoRequest exchange. The raw bytes match
/// what tesla-control writes into its session-cache JSON, so we
/// can mirror its on-disk format byte-for-byte.
#[derive(Debug, Clone)]
pub struct SessionInfoResponse {
    pub domain: Domain,
    /// Raw SessionInfo proto bytes — keep for cache-file compat.
    pub raw: Vec<u8>,
    pub parsed: SessionInfo,
    /// The 16-byte request UUID we used as the challenge for this
    /// exchange. Needed by the caller if it wants to verify the
    /// SessionInfo HMAC tag (the car HMAC-signs over the challenge
    /// + encoded_info, see auth::compute_session_info_hmac).
    pub challenge: [u8; 16],
    /// The HMAC tag the car attached in `sub_sig_data.SessionInfoTag`,
    /// or None if the response didn't include one. Callers should
    /// reject `None` for security-sensitive paths — an absent tag
    /// means the SessionInfo isn't authenticated.
    pub session_info_tag: Option<Vec<u8>>,
}

/// Send SessionInfoRequest to `domain` and decode the response.
///
/// Returns `SessionError::KeyNotPaired` if the car responds with
/// `SESSION_INFO_STATUS_KEY_NOT_ON_WHITELIST` — distinct from generic
/// transport/decode failures so the caller can render a clear "re-pair
/// your card" error in the UI instead of "AES-GCM decrypt failed three
/// minutes later because we tried to use an unauthorized session."
pub async fn request_session_info(
    conn: &mut Connection,
    keypair: &KeyPair,
    domain: Domain,
) -> std::result::Result<SessionInfoResponse, SessionError> {
    let (payload, challenge) = build_request(keypair, domain);
    debug!(
        "session-info: TX {} bytes to {:?} (challenge={})",
        payload.len(),
        domain,
        hex::encode(challenge),
    );
    // Validator: must decode as a RoutableMessage. Same rationale
    // as the other round_trip callers; the session-info handshake
    // runs right after a fresh connect when stale notifications
    // are most likely to be in the BLE pipeline.
    let response = conn
        .round_trip(&payload, Duration::from_secs(10), |b| {
            RoutableMessage::decode(b).is_ok()
        })
        .await
        .context("session-info round-trip")?;
    debug!("session-info: RX {} bytes", response.len());
    parse_response(&response, domain, challenge)
}

/// Build a SessionInfoRequest payload AND return the request UUID we
/// used as the challenge. The challenge is required by the caller for
/// SessionInfo HMAC verification: the car binds its response to this
/// specific UUID, and replaying an old SessionInfo from a previous
/// handshake should fail HMAC verification.
fn build_request(keypair: &KeyPair, domain: Domain) -> (Vec<u8>, [u8; 16]) {
    let from_uuid = random_uuid_bytes();
    let req_uuid = random_uuid_bytes();
    let msg = RoutableMessage {
        to_destination: Some(Destination {
            sub_destination: Some(destination::SubDestination::Domain(domain as i32)),
        }),
        from_destination: Some(Destination {
            sub_destination: Some(destination::SubDestination::RoutingAddress(
                from_uuid.to_vec(),
            )),
        }),
        payload: Some(routable_message::Payload::SessionInfoRequest(
            SessionInfoRequest {
                public_key: keypair.pub_uncompressed.clone(),
                challenge: Vec::new(),
            },
        )),
        uuid: req_uuid.to_vec(),
        ..Default::default()
    };
    (msg.encode_to_vec(), req_uuid)
}

fn parse_response(
    bytes: &[u8],
    domain: Domain,
    challenge: [u8; 16],
) -> std::result::Result<SessionInfoResponse, SessionError> {
    use crate::proto::signatures::signature_data;

    debug!("session-info RX hex: {}", hex::encode(bytes));
    let routable = RoutableMessage::decode(bytes)
        .context("decode outer Routable")?;
    debug!("session-info RX decoded: {:#?}", routable);

    // Pull out the HMAC tag from sub_sig_data.SessionInfoTag, if
    // present. We don't reject here on missing-tag — that's a
    // policy decision the caller (manager.rs) makes, since the
    // initial pair-time bootstrap may need to accept untagged
    // session info from older firmware before HMAC keys are
    // established.
    let session_info_tag = routable.sub_sig_data.as_ref().and_then(|s| match s {
        crate::proto::universal_message::routable_message::SubSigData::SignatureData(sd) => {
            match sd.sig_type.as_ref() {
                Some(signature_data::SigType::SessionInfoTag(hmac_sig)) => {
                    Some(hmac_sig.tag.clone())
                }
                _ => None,
            }
        }
    });

    let raw = match routable.payload {
        Some(routable_message::Payload::SessionInfo(b)) => b,
        Some(other) => {
            return Err(SessionError::Other(anyhow::anyhow!(
                "expected session_info payload, got {:?}",
                other
            )));
        }
        None => {
            return Err(SessionError::Other(anyhow::anyhow!(
                "response has no payload (signed_message_status={:?}, to={:?}, from={:?})",
                routable.signed_message_status,
                routable.to_destination,
                routable.from_destination
            )));
        }
    };
    let parsed = SessionInfo::decode(raw.as_slice())
        .context("decode SessionInfo proto")?;

    // Promote KEY_NOT_ON_WHITELIST to a distinct error so the UI can
    // tell the user to re-pair their card. Matches tesla-control's
    // `protocol.GetError` behavior. Without this check the bytes
    // would parse fine, we'd derive a useless session key, and the
    // next encrypted query would fail with AES-GCM decrypt error —
    // a confusing failure mode that hides the actual problem.
    if parsed.status == SessionInfoStatus::KeyNotOnWhitelist as i32 {
        warn!(
            "session-info from {:?}: car returned KEY_NOT_ON_WHITELIST — \
             our public key (sha256 prefix {}) is not paired with this VIN. \
             User must re-pair from the SentryUSB UI and tap the card on the console.",
            domain,
            // Show a short fingerprint of our pubkey so the user can
            // cross-check against the pair output if needed.
            "<see /root/.ble/key_public.pem>"
        );
        return Err(SessionError::KeyNotPaired);
    }

    info!(
        "session-info from {:?}: counter={}, clock_time={}, pubkey={} bytes, \
         status={}, hmac_tag={}",
        domain,
        parsed.counter,
        parsed.clock_time,
        parsed.public_key.len(),
        parsed.status,
        match &session_info_tag {
            Some(t) => format!("{} bytes", t.len()),
            None => "<missing>".to_string(),
        },
    );
    Ok(SessionInfoResponse {
        domain,
        raw,
        parsed,
        challenge,
        session_info_tag,
    })
}

fn random_uuid_bytes() -> [u8; 16] {
    let mut out = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut out);
    out
}
