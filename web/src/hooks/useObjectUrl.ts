import { useEffect, useMemo } from "react"

export function useObjectUrl(file: File): string {
  const url = useMemo(() => URL.createObjectURL(file), [file])
  useEffect(() => {
    return () => URL.revokeObjectURL(url)
  }, [url])
  return url
}
