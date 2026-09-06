// patch script
import fs from "node:fs";

let dash = fs.readFileSync("src/application/dashboard.ts", "utf8");

// 1. Add auth imports
const mcpImport = `import { getMcpStats, listMcpCalls } from "./mcp-monitor.js";`;
const authImport = `import {
  ensureDefaultAuth,
  verifyCredentials,
  changePassword,
  createSession,
  validateSession,
  destroySession,
  extractSessionToken,
} from "../infrastructure/dashboard-auth.js";`;

dash = dash.replace(mcpImport, mcpImport + "\n" + authImport);
console.log("Step 1: imports added");

// 2. Add render functions after renderFooter
const renderFooterEnd = `function renderFooter(): string {
  return \`<div class="footer">
  <p>Harness v\${htmlEscape(VERSION)} &mdash; <a href="https://github.com/vantanminh/5harness">github.com/vantanminh/5harness</a></p>
</div>\`;
}`;

const newRenderFns = `

function renderLoginPage(error?: string): string {
  const errorHtml = error
    ? \`<p style="color:var(--status-missing);margin-bottom:1rem">\${htmlEscape(error)}</p>\`
    : "";
  return \`<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Login — Harness Dashboard</title>
  <style>\${CSS}
    .login-box { max-width: 360px; margin: 5rem auto; padding: 2rem; border: 1px solid var(--border); border-radius: 8px; background: var(--th-bg); }
    .login-box h1 { margin-top: 0; font-size: 1.3rem; text-align: center; }
    .login-box label { display: block; margin-top: 0.75rem; font-weight: 600; font-size: 0.85rem; }
    .login-box input[type="text"],
    .login-box input[type="password"] { width: 100%; padding: 0.5rem; margin-top: 0.25rem; border: 1px solid var(--border); border-radius: 4px; background: var(--bg); color: var(--fg); font-size: 0.95rem; }
    .login-box button { width: 100%; margin-top: 1.25rem; padding: 0.6rem; background: var(--link); color: #fff; border: none; border-radius: 4px; font-size: 1rem; cursor: pointer; }
    .login-box button:hover { opacity: 0.9; }
    .login-box .hint { margin-top: 1rem; font-size: 0.8rem; color: var(--muted); text-align: center; }
  </style>
</head>
<body>
  <div class="login-box">
    <h1>Harness Dashboard</h1>
    \${errorHtml}
    <form method="post" action="/api/auth/login">
      <label for="username">Username</label>
      <input type="text" id="username" name="username" value="admin" autocomplete="username" />
      <label for="password">Password</label>
      <input type="password" id="password" name="password" autocomplete="current-password" />
      <button type="submit">Sign in</button>
    </form>
    <p class="hint">There is no default password. Set one with <code>harness dashboard set-password</code> before exposing the dashboard.</p>
  </div>
</body>
</html>\`;
}

function renderSettings(
  success?: string,
  error?: string,
): string {
  const msgHtml = success
    ? \`<p style="color:var(--status-ok);margin-bottom:1rem">\${htmlEscape(success)}</p>\`
    : error
      ? \`<p style="color:var(--status-missing);margin-bottom:1rem">\${htmlEscape(error)}</p>\`
      : "";
  return \`<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Settings — Harness Dashboard</title>
  <style>\${CSS}
    .settings-box { max-width: 420px; margin: 2rem auto; padding: 2rem; border: 1px solid var(--border); border-radius: 8px; background: var(--th-bg); }
    .settings-box h1 { margin-top: 0; font-size: 1.3rem; }
    .settings-box label { display: block; margin-top: 0.75rem; font-weight: 600; font-size: 0.85rem; }
    .settings-box input[type="password"] { width: 100%; padding: 0.5rem; margin-top: 0.25rem; border: 1px solid var(--border); border-radius: 4px; background: var(--bg); color: var(--fg); font-size: 0.95rem; }
    .settings-box button { margin-top: 1.25rem; padding: 0.6rem 1.5rem; background: var(--link); color: #fff; border: none; border-radius: 4px; font-size: 0.95rem; cursor: pointer; }
    .settings-box button:hover { opacity: 0.9; }
  </style>
</head>
<body>
  <div class="nav">
    <a href="/">&larr; Dashboard</a>
    <a href="/settings">Settings</a>
  </div>
  <div class="settings-box">
    <h1>Change Password</h1>
    \${msgHtml}
    <form method="post" action="/api/auth/change-password">
      <label for="currentPassword">Current password</label>
      <input type="password" id="currentPassword" name="currentPassword" autocomplete="current-password" required />
      <label for="newPassword">New password</label>
      <input type="password" id="newPassword" name="newPassword" autocomplete="new-password" required minlength="1" />
      <button type="submit">Update password</button>
    </form>
  </div>
  \${renderFooter()}
</body>
</html>\`;
}

function renderLoggedOut(): string {
  return \`<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Logged out — Harness Dashboard</title>
  <style>\${CSS}</style>
</head>
<body>
  <h1>Logged out</h1>
  <p>You have been signed out of the dashboard.</p>
  <p><a href="/login">Sign in again</a></p>
  \${renderFooter()}
</body>
</html>\`;
}`;

dash = dash.replace(renderFooterEnd, renderFooterEnd + newRenderFns);
console.log("Step 2: render functions added");


// 3. Add auth middleware + routes in startDashboard
const urlParseLine = `      const url = new URL(req.url ?? "/", \`http://\${host}:\${port}\`);`;

const authMiddleware = `
      const authHomeOpts = dashOpts.harnessHome ? { harnessHome: dashOpts.harnessHome } : undefined;
      ensureDefaultAuth(authHomeOpts);

      // Auth: public paths (login, auth API) skip auth check
      const publicPaths = ["/login", "/api/auth/login", "/api/auth/logout"];
      const isPublic = publicPaths.includes(url.pathname);

      // Auth: session check for protected paths
      if (!isPublic) {
        const cookieHeader = (req.headers.cookie ?? req.headers["cookie"]) as string | undefined;
        const token = extractSessionToken(cookieHeader);
        if (!token || !validateSession(token)) {
          if (url.pathname.startsWith("/api/")) {
            res.writeHead(401, { "Content-Type": "application/json; charset=utf-8" });
            res.end(JSON.stringify({ error: "Unauthorized" }));
            return;
          }
          res.writeHead(302, { "Location": "/login?redirect=" + encodeURIComponent(req.url ?? "/") });
          res.end();
          return;
        }
      }

      // Auth routes
      if (url.pathname === "/api/auth/login" && req.method === "POST") {
        let body = "";
        req.on("data", (chunk: Buffer) => { body += chunk.toString(); });
        req.on("end", () => {
          try {
            const params = new URLSearchParams(body);
            const username = params.get("username") ?? "";
            const password = params.get("password") ?? "";
            const isValid = verifyCredentials(username, password, authHomeOpts);
            if (isValid) {
              const session = createSession();
              const redirect = new URL(url.searchParams.get("redirect") ?? "/", \`http://\${host}:\${port}\`).pathname;
              res.writeHead(302, {
                "Location": redirect,
                "Set-Cookie": \`harness_session=\${session.token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=\${Math.floor((session.expiresAt - Date.now()) / 1000)}\`,
              });
              res.end();
            } else {
              res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
              res.end(renderLoginPage("Invalid username or password"));
            }
          } catch (err) {
            res.writeHead(500, { "Content-Type": "text/plain; charset=utf-8" });
            res.end(err instanceof Error ? err.message : String(err));
          }
        });
        return;
      }

      if (url.pathname === "/api/auth/logout" && req.method === "POST") {
        const cookieHeader = (req.headers.cookie ?? req.headers["cookie"]) as string | undefined;
        const token = extractSessionToken(cookieHeader);
        if (token) destroySession(token);
        res.writeHead(302, {
          "Location": "/login",
          "Set-Cookie": "harness_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"
        });
        res.end();
        return;
      }

      if (url.pathname === "/api/auth/change-password" && req.method === "POST") {
        let body = "";
        req.on("data", (chunk: Buffer) => { body += chunk.toString(); });
        req.on("end", () => {
          try {
            const params = new URLSearchParams(body);
            const currentPassword = params.get("currentPassword") ?? "";
            const newPassword = params.get("newPassword") ?? "";
            if (!newPassword) {
              res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
              res.end(renderSettings(undefined, "New password must not be empty"));
              return;
            }
            const ok = changePassword("admin", currentPassword, newPassword, authHomeOpts);
            if (ok) {
              res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
              res.end(renderSettings("Password updated successfully."));
            } else {
              res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
              res.end(renderSettings(undefined, "Current password is incorrect"));
            }
          } catch (err) {
            res.writeHead(500, { "Content-Type": "text/plain; charset=utf-8" });
            res.end(err instanceof Error ? err.message : String(err));
          }
        });
        return;
      }

      // Login page
      if (url.pathname === "/login") {
        res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
        res.end(renderLoginPage());
        return;
      }

      // Settings page
      if (url.pathname === "/settings") {
        res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
        res.end(renderSettings());
        return;
      }
`;

dash = dash.replace(urlParseLine, urlParseLine + authMiddleware);
console.log("Step 3: auth middleware inserted");

// 4. Add nav/logout to home page renderHome (second <h1>Harness Dashboard</h1>)
const homePageH1 = `  <h1>Harness Dashboard</h1>
  <p class="muted">Local read-only view of registry projects (markdown SoT).</p>`;

const homeNavBlock = `  <h1>Harness Dashboard</h1>
  <div class="nav">
    <a href="/settings">Settings</a>
    <form method="post" action="/api/auth/logout" style="display:inline">
      <button type="submit" style="background:none;border:none;color:var(--link);cursor:pointer;font-size:0.9rem;padding:0;text-decoration:none;font-family:inherit">Logout</button>
    </form>
  </div>
  <p class="muted">Local read-only view of registry projects (markdown SoT).</p>`;

// Find the second occurrence (first is in renderLoginPage)
const firstH1 = dash.indexOf(homePageH1);
const secondH1 = dash.indexOf(homePageH1, firstH1 + 1);

if (secondH1 > firstH1) {
  const prefix = dash.substring(0, secondH1);
  const suffix = dash.substring(secondH1 + homePageH1.length);
  dash = prefix + homeNavBlock + suffix;
  console.log("Step 4: home nav added");
} else {
  console.log("WARNING: second homePageH1 not found, firstH1=" + firstH1 + " secondH1=" + secondH1);
}

fs.writeFileSync("src/application/dashboard.ts", dash, "utf8");
console.log("All steps complete.");
