/**
 * Turn a task's recorded error into a sentence a person can act on.
 *
 * Errors are stored as the engine produced them — the full gRPC status
 * with its kind, message and retry hint — because that is what a bug
 * report needs. It is not what a download list should show: nobody
 * reading "Server(Grpc { kind: Unauthenticated, … })" learns that
 * another app signed in to their account.
 *
 * The raw text stays available (callers put it in a tooltip); this is
 * only the headline.
 */
const KNOWN: { test: RegExp; say: string }[] = [
  {
    test: /flatfiles unavailable: market-data auth rejected/i,
    say: "Flat files need your account email — sign in again with it alongside your API key",
  },
  {
    test: /invalid session|unauthenticated/i,
    say: "Signed out — another app signed in to this ThetaData account",
  },
  {
    test: /too many days between start and end/i,
    say: "Window wider than the server allows",
  },
  {
    test: /cannot specify '\*'/i,
    say: "This dataset needs a specific expiration",
  },
  {
    test: /interval must be positive/i,
    say: "This dataset has no tick-level granularity",
  },
  {
    test: /subscription|upgrade|permissiondenied/i,
    say: "Not included in your subscription",
  },
  { test: /timeout|deadline/i, say: "The server took too long to answer" },
  { test: /unavailable|connection (reset|refused)|broken pipe/i, say: "Connection to ThetaData dropped" },
  { test: /missing required arg '([^']+)'/i, say: "Request was missing '$1'" },
];

export function friendlyError(raw: string): string {
  const split = /\(re-queued as (\d+) narrower windows\)/i.exec(raw);
  const suffix = split ? ` — retrying as ${split[1]} smaller windows` : "";

  for (const { test, say } of KNOWN) {
    const m = test.exec(raw);
    if (m) return say.replace("$1", m[1] ?? "") + suffix;
  }
  // Fall back to the server's own sentence, stripped of the envelope.
  const inner = /message:\s*"([^"]+)"/.exec(raw);
  if (inner) return inner[1].split("\n")[0] + suffix;
  return raw;
}
