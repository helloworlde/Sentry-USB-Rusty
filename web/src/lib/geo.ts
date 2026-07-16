/**
 * Canonical longitude in [-180, 180). Leaflet allows panning into repeated
 * world copies, so a click on Japan can report lng ≈ -221.4 (138.6 - 360);
 * wrap before display/storage so coordinates stay canonical. Matches the
 * backend's `normalize_lon` (same half-open range).
 */
export function normalizeLon(lon: number): number {
  return ((((lon + 180) % 360) + 360) % 360) - 180
}
