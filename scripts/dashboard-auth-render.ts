function renderLoginPage(error?: string): string {
  const errorHtml = error
    ? `<p style="color:var(--status-missing);margin-bottom:1rem">${htmlEscape(error)}</p>`
    : "";
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Login — Harness Dashboard</title>
  <style>${CSS}
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
    ${errorHtml}
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
</html>`;
}

function renderSettings(
  success?: string,
  error?: string,
): string {
  const msgHtml = success
    ? `<p style="color:var(--status-ok);margin-bottom:1rem">${htmlEscape(success)}</p>`
    : error
      ? `<p style="color:var(--status-missing);margin-bottom:1rem">${htmlEscape(error)}</p>`
      : "";
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Settings — Harness Dashboard</title>
  <style>${CSS}
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
    ${msgHtml}
    <form method="post" action="/api/auth/change-password">
      <label for="currentPassword">Current password</label>
      <input type="password" id="currentPassword" name="currentPassword" autocomplete="current-password" required />
      <label for="newPassword">New password</label>
      <input type="password" id="newPassword" name="newPassword" autocomplete="new-password" required minlength="1" />
      <button type="submit">Update password</button>
    </form>
  </div>
  ${renderFooter()}
</body>
</html>`;
}

function renderLoggedOut(): string {
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Logged out — Harness Dashboard</title>
  <style>${CSS}</style>
</head>
<body>
  <h1>Logged out</h1>
  <p>You have been signed out of the dashboard.</p>
  <p><a href="/login">Sign in again</a></p>
  ${renderFooter()}
</body>
</html>`;
}
