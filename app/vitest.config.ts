import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["src/**/*.test.{ts,tsx}"],
    // Step 1 has no tests yet; an empty suite is a pass, not an error.
    passWithNoTests: true,
  },
});
