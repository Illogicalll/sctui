// sctui auth relay.
//
// SoundCloud requires the client secret on every token request, even with PKCE, so a native
// app cannot talk to the token endpoint directly without shipping the secret. This Worker holds
// the secret and forwards exactly two requests: the initial code exchange and token refresh.
//
// It is stateless. It never parses, stores or logs SoundCloud's reply: the upstream Response is
// returned to the client as-is.

const TOKEN_URL = "https://secure.soundcloud.com/oauth/token";
// Must match the redirect URI the client used in its /authorize request (src/auth/oauth.rs).
const REDIRECT_URI = "http://127.0.0.1:8080/callback";

// Only these two grants are ever forwarded, and only these client-supplied fields are copied.
// Everything else in the incoming body is dropped, so a caller cannot pick another grant type.
const ROUTES = {
  "/token": { grant_type: "authorization_code", fields: ["code", "code_verifier"] },
  "/refresh": { grant_type: "refresh_token", fields: ["refresh_token"] },
};

/** Body to send upstream for `pathname`. Throws with a client-safe message on bad input. */
export function forwardBody(pathname, form, env) {
  const route = ROUTES[pathname];
  if (!route) throw new Error("not found");
  const body = new URLSearchParams({
    grant_type: route.grant_type,
    client_id: env.SOUNDCLOUD_CLIENT_ID,
    client_secret: env.SOUNDCLOUD_CLIENT_SECRET,
  });
  if (route.grant_type === "authorization_code") body.set("redirect_uri", REDIRECT_URI);
  for (const field of route.fields) {
    const value = form.get(field);
    if (!value) throw new Error(`missing ${field}`);
    body.set(field, value);
  }
  return body;
}

export default {
  async fetch(request, env) {
    const { pathname } = new URL(request.url);
    if (request.method === "GET" && pathname === "/version") {
      return new Response(env.GIT_SHA ?? "dev");
    }
    if (request.method !== "POST") return new Response("method not allowed", { status: 405 });
    if (!(pathname in ROUTES)) return new Response("not found", { status: 404 });

    let body;
    try {
      body = forwardBody(pathname, new URLSearchParams(await request.text()), env);
    } catch (e) {
      return new Response(e.message, { status: 400 });
    }

    return fetch(TOKEN_URL, {
      method: "POST",
      headers: {
        "content-type": "application/x-www-form-urlencoded",
        accept: "application/json; charset=utf-8",
      },
      body,
    });
  },
};
