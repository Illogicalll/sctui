// Run: node --test test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { forwardBody } from "./src/index.js";

const env = { SOUNDCLOUD_CLIENT_ID: "id", SOUNDCLOUD_CLIENT_SECRET: "sec" };
const form = (s) => new URLSearchParams(s);

test("/token copies code + verifier, adds secret and fixed redirect, drops everything else", () => {
  const body = forwardBody("/token", form("code=c&code_verifier=v&grant_type=client_credentials&client_id=evil"), env);
  assert.deepEqual(Object.fromEntries(body), {
    grant_type: "authorization_code",
    client_id: "id",
    client_secret: "sec",
    redirect_uri: "http://127.0.0.1:8080/callback",
    code: "c",
    code_verifier: "v",
  });
});

test("/refresh copies refresh_token only", () => {
  const body = forwardBody("/refresh", form("refresh_token=r&scope=*"), env);
  assert.deepEqual(Object.fromEntries(body), {
    grant_type: "refresh_token",
    client_id: "id",
    client_secret: "sec",
    refresh_token: "r",
  });
});

test("missing field is rejected", () => {
  assert.throws(() => forwardBody("/token", form("code=c"), env), /missing code_verifier/);
  assert.throws(() => forwardBody("/refresh", form(""), env), /missing refresh_token/);
});

test("unknown path is rejected", () => {
  assert.throws(() => forwardBody("/oauth/token", form("grant_type=client_credentials"), env), /not found/);
});
