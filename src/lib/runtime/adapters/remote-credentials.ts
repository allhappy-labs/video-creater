let csrfToken: string | null = null;

export function setRemoteCsrfToken(token: string): void {
  csrfToken = token;
}

export function remoteCsrfToken(fallback: string): string {
  return csrfToken ?? fallback;
}

export function resetRemoteCsrfTokenForTests(): void {
  csrfToken = null;
}
