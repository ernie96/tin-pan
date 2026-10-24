// Tin Pan report relay: takes a report from Report a problem (Ctrl+Shift+B) or Suggest a feature (Ctrl+Shift+F) and files it as an
// issue in a private GitHub repository. Tin Pan users need no account, and the GitHub token stays here.
//
// Settings (see README.md):
//   REPORTS_REPO   variable, "owner/name" of the private repository
//   GITHUB_TOKEN   secret, a fine-grained token for that repository only: Issues and Contents, read and write
//   LIMITER        rate limit binding (wrangler.toml)

const MAX_DESCRIPTION = 5000;
const MAX_CONTACT = 200;
const MAX_LOG = 400000; // characters; Tin Pan sends at most 200,000
const LOG_IN_BODY = 50000; // GitHub issues hold about 65,000 characters; longer logs go in a file

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (request.method !== "POST" || url.pathname !== "/report")
      return json({ error: "Not found" }, 404);
    if (request.headers.get("X-TinPan-Report") !== "1")
      return json({ error: "Not a Tin Pan report" }, 400);

    const ip = request.headers.get("CF-Connecting-IP") || "unknown";
    if (env.LIMITER) {
      const { success } = await env.LIMITER.limit({ key: ip });
      if (!success) return json({ error: "Too many reports; wait a minute." }, 429);
    }

    const size = Number(request.headers.get("Content-Length") || 0);
    if (size > 1200000) return json({ error: "Report too large" }, 413);

    let report;
    try { report = await request.json(); } catch { return json({ error: "Invalid report" }, 400); }

    const description = text(report.description, MAX_DESCRIPTION);
    if (description.trim().length < 3) return json({ error: "Describe the problem" }, 400);
    const contact = text(report.contact, MAX_CONTACT);
    const version = text(report.version, 40);
    const language = text(report.language, 10);
    const windows = text(report.windows, 120);
    const log = text(report.log, MAX_LOG);
    const feature = report.kind === "feature";

    const firstLine = description.split(/\r?\n/)[0].trim();
    const shortLine = firstLine.length > 80 ? firstLine.slice(0, 77) + "..." : firstLine;
    const title = `[${feature ? "Feature, " : ""}${version || "?"}] ${shortLine.replace(/@/g, "@\u200b")}`;

    let body =
      `${quote(description)}\n\n` +
      `| | |\n|---|---|\n` +
      `| Version | ${cell(version)} |\n` +
      `| Language | ${cell(language)} |\n` +
      `| Windows | ${cell(windows)} |\n` +
      `| Contact | ${contact ? cell(contact) : "none given"} |\n`;

    const labels = [feature ? "feature request" : "report"];
    if (log) {
      if (log.length <= LOG_IN_BODY) {
        body += `\n<details><summary>Log (${log.length.toLocaleString("en")} characters)</summary>\n\n${fence(log)}\n</details>\n`;
      } else {
        const path = `logs/${new Date().toISOString().slice(0, 10)}-${crypto.randomUUID()}.log`;
        const saved = await github(env, `contents/${path}`, "PUT", {
          message: `Log for report: ${title}`,
          content: base64(log),
        });
        body += saved.ok
          ? `\nLog: [${path}](https://github.com/${env.REPORTS_REPO}/blob/main/${path}) (${log.length.toLocaleString("en")} characters)\n`
          : `\nThe log could not be saved (${saved.status}); its end:\n\n${fence(log.slice(-LOG_IN_BODY))}\n`;
      }
      labels.push("has log");
    }

    const issue = await github(env, "issues", "POST", { title, body, labels });
    if (!issue.ok) return json({ error: `GitHub answered ${issue.status}` }, 502);
    const created = await issue.json();
    return json({ number: created.number }, 201);
  },
};

function text(value, max) {
  return typeof value === "string" ? value.replace(/\u0000/g, "").slice(0, max) : "";
}

// Report text is shown as quoted text, so it can't mention people or close issues by accident.
function quote(value) {
  return value.replace(/@/g, "@\u200b").split(/\r?\n/).map((line) => "> " + line).join("\n");
}

function cell(value) {
  return value.replace(/[|\r\n]/g, " ").replace(/@/g, "@\u200b");
}

function fence(value) {
  const ticks = "`".repeat(Math.max(3, ...[...value.matchAll(/`+/g)].map((m) => m[0].length + 1)));
  return `${ticks}text\n${value}\n${ticks}`;
}

function base64(value) {
  const bytes = new TextEncoder().encode(value);
  let binary = "";
  for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(binary);
}

function github(env, path, method, payload) {
  return fetch(`https://api.github.com/repos/${env.REPORTS_REPO}/${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${env.GITHUB_TOKEN}`,
      Accept: "application/vnd.github+json",
      "User-Agent": "TinPan-report-relay",
      "X-GitHub-Api-Version": "2022-11-28",
    },
    body: JSON.stringify(payload),
  });
}

function json(value, status) {
  return new Response(JSON.stringify(value), { status, headers: { "Content-Type": "application/json" } });
}
