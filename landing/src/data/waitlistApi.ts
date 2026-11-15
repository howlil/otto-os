export type WaitlistApiResult = {
  ok: boolean;
  token?: string;
  error?: string;
};

export type JoinWaitlistInput = {
  email: string;
  website: string;
  source: "hero";
};

export type SaveProfileInput = {
  email: string;
  token: string;
  platform: string;
  usage: string;
};

async function postWaitlist(body: Record<string, unknown>): Promise<WaitlistApiResult> {
  const response = await fetch("/api/waitlist", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  return (await response.json()) as WaitlistApiResult;
}

export async function joinWaitlist(input: JoinWaitlistInput): Promise<WaitlistApiResult> {
  return postWaitlist({
    email: input.email,
    website: input.website,
    source: input.source,
  });
}

export async function saveWaitlistProfile(input: SaveProfileInput): Promise<WaitlistApiResult> {
  return postWaitlist({
    email: input.email,
    token: input.token,
    profile: true,
    platform: input.platform,
    usage: input.usage,
  });
}
