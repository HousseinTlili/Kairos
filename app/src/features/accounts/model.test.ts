import { describe, expect, it } from "vitest";
import { disconnectQuestion, googleSetupRequest, statusText } from "./model";

describe("accounts wording", () => {
  it("shows the reason when a sign-in route isn't installed", () => {
    expect(
      statusText({ kind: "github", status: "unavailable", reason: "gh isn't installed." }),
    ).toBe("gh isn't installed.");
    expect(statusText({ kind: "google", status: "connected" })).toBe("Connected");
  });

  it("asks before disconnecting and says what is lost", () => {
    expect(disconnectQuestion("google")).toMatch(/revoked/);
    expect(disconnectQuestion("github")).toMatch(/signs out/);
  });

  it("connects Google through the skill's OAuth, for Gmail and Calendar only, never a password", () => {
    const text = googleSetupRequest();
    expect(text).toContain("email and calendar");
    expect(text).toContain("OAuth");
    expect(text).toMatch(/Never ask for my Google password/);
  });
});
