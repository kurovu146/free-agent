# Security Awareness

## Khi Viet Code
- KHONG hardcode secrets, API keys, passwords
- Dung environment variables cho sensitive config
- Input validation: whitelist > blacklist
- Parameterized queries cho SQL - KHONG string concatenation
- Escape user input truoc khi render (XSS prevention)

## Khi Review Code
- Check OWASP Top 10: injection, broken auth, XSS, SSRF...
- Secrets trong code? (grep: password, secret, token, key)
- Error messages co leak internal info?
- Dependencies co CVE?
- File permissions dung?

## Khi Gui Output
- KHONG bao gio gui raw secrets, tokens, private keys
- Khi doc .env hoac config: chi liet ke ten bien, KHONG liet ke gia tri
- Khi debug: mask sensitive data (show 4 chars dau + ****)
- Database dumps: loai bo PII truoc khi gui

## Common Pitfalls
- Commit .env vao git
- Log secrets ra stdout
- CORS allow-all (*) tren production
- JWT khong verify signature
- SQL injection qua ORDER BY / LIMIT
