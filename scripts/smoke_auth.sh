#!/usr/bin/env bash
set -euo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
for bin in curl jq; do
  command -v "$bin" >/dev/null 2>&1 || {
    echo "Missing required command: $bin" >&2
    exit 1
  }
done

post_json() {
  local path="$1"
  local payload="$2"
  shift 2
  curl -fsS -H 'content-type: application/json' "$@" -d "$payload" "$API_BASE_URL$path"
}

suffix="$(date +%s)-${RANDOM}${RANDOM}"
email_a="auth-a-${suffix}@example.com"
email_b="auth-b-${suffix}@example.com"

owner_a="$(post_json /api/v1/auth/register "$(jq -cn --arg email "$email_a" '{email:$email,display_name:"Auth A",password:"password-a-123"}')")"
owner_b="$(post_json /api/v1/auth/register "$(jq -cn --arg email "$email_b" '{email:$email,display_name:"Auth B",password:"password-b-123"}')")"

owner_a_id="$(jq -r '.user.id' <<<"$owner_a")"
owner_a_token="$(jq -r '.session_token' <<<"$owner_a")"
owner_b_token="$(jq -r '.session_token' <<<"$owner_b")"

me="$(curl -fsS -H "authorization: Bearer $owner_a_token" "$API_BASE_URL/api/v1/auth/me")"
jq -e --arg id "$owner_a_id" '.user.id == $id' >/dev/null <<<"$me"

sessions="$(curl -fsS -H "authorization: Bearer $owner_a_token" "$API_BASE_URL/api/v1/auth/sessions")"
jq -e '.sessions | length >= 1 and any(.is_current == true)' >/dev/null <<<"$sessions"

verification="$(post_json /api/v1/auth/email-verification/request '{}' -H "authorization: Bearer $owner_a_token")"
verification_token="$(jq -r '.development_token' <<<"$verification")"
post_json /api/v1/auth/email-verification/verify "$(jq -cn --arg token "$verification_token" '{token:$token}')" >/dev/null

company="$(post_json /api/v1/companies \
  "$(jq -cn --arg slug "security-$suffix" '{name:"Security Smoke Company",slug:$slug,description:"tenant boundary smoke"}')" \
  -H "authorization: Bearer $owner_a_token")"
company_id="$(jq -r '.company_console.company.id' <<<"$company")"

cross_status="$(curl -sS -o /tmp/ai_chat_cross_owner.json -w '%{http_code}' \
  -H "authorization: Bearer $owner_b_token" \
  "$API_BASE_URL/api/v1/companies/$company_id/console")"
[[ "$cross_status" == "401" ]]

admin_status="$(curl -sS -o /tmp/ai_chat_admin_console.json -w '%{http_code}' \
  "$API_BASE_URL/api/v1/admin/console")"
[[ "$admin_status" == "404" ]]
jq -e '.code == "not_found"' >/dev/null </tmp/ai_chat_admin_console.json

post_json /api/v1/auth/password/change \
  '{"current_password":"password-a-123","new_password":"password-a-456"}' \
  -H "authorization: Bearer $owner_a_token" >/dev/null

old_login_status="$(curl -sS -o /tmp/ai_chat_old_login.json -w '%{http_code}' \
  -H 'content-type: application/json' \
  -d "$(jq -cn --arg email "$email_a" '{email:$email,password:"password-a-123"}')" \
  "$API_BASE_URL/api/v1/auth/login")"
[[ "$old_login_status" == "401" ]]

new_login="$(post_json /api/v1/auth/login "$(jq -cn --arg email "$email_a" '{email:$email,password:"password-a-456"}')")"
jq -e '.session_token | startswith("hus_")' >/dev/null <<<"$new_login"

reset_request="$(post_json /api/v1/auth/password/reset/request "$(jq -cn --arg email "$email_a" '{email:$email}')")"
reset_token="$(jq -r '.development_token' <<<"$reset_request")"
post_json /api/v1/auth/password/reset/confirm "$(jq -cn --arg token "$reset_token" '{token:$token,new_password:"password-a-789"}')" >/dev/null

if command -v psql >/dev/null 2>&1 && [[ -n "${DATABASE_URL:-}" ]]; then
  plaintext_rows="$(psql "$DATABASE_URL" -Atqc "SELECT COUNT(*) FROM agent_keys WHERE key_name LIKE 'plaintext:%';")"
  [[ "$plaintext_rows" == "0" ]]
fi

rm -f /tmp/ai_chat_cross_owner.json /tmp/ai_chat_admin_console.json /tmp/ai_chat_old_login.json
echo "Authentication and security smoke completed successfully."
