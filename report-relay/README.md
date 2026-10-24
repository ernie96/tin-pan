# Tin Pan report relay

Report a problem (Ctrl+Shift+B) and Suggest a feature (Ctrl+Shift+F) in Tin Pan send here. This small Cloudflare Worker files each report as an issue in the private repository **ernie96/tin-pan-reports**, so Tin Pan users need no account, and their logs aren't public. Logs longer than about 50,000 characters are saved as files in that repository's `logs` folder and linked from the issue.

Limits: 3 reports a minute from one internet address, 5,000 characters of description, 400,000 of log.

## One-time setup

1. **Reports repository.** Create the private repository **ernie96/tin-pan-reports** on GitHub, with labels `report`, `feature request` and `has log` (the relay adds them; GitHub creates missing labels itself).
2. **GitHub token.** On GitHub: Settings, Developer settings, Personal access tokens, Fine-grained tokens, Generate new token. Name it "Tin Pan reports". Repository access: Only select repositories, **ernie96/tin-pan-reports**. Permissions: **Issues: Read and write**, **Contents: Read and write**. Copy the token.
3. **Deploy**, from this folder, with your Cloudflare account:

   ```
   npx wrangler login
   npx wrangler deploy
   npx wrangler secret put GITHUB_TOKEN
   ```

   Paste the token when asked. `deploy` prints the address, such as `https://tinpan-reports.ernie96.workers.dev`.
4. If that address differs from `RelayUrl` in `TinPan/Services/ReportService.cs`, change it there (keep `/report` at the end).

Until the relay is deployed, sending a report says it couldn't reach the report service; nothing else is affected.

## Reading reports

```
gh issue list --repo ernie96/tin-pan-reports
gh issue list --repo ernie96/tin-pan-reports --label "feature request"
gh issue view 12 --repo ernie96/tin-pan-reports
```
