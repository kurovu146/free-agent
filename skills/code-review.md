# Code Review

## Review Priority (cao -> thap)
1. **Security** - injection, XSS, secrets, auth bypass
2. **Correctness** - logic bugs, edge cases, race conditions
3. **Performance** - N+1 queries, memory leaks, hot paths
4. **Maintainability** - naming, structure, complexity
5. **Style** - formatting, conventions

## Security Checklist
- SQL Injection: parameterized queries, KHONG string concat
- XSS: escape user input truoc khi render
- Auth bypass: kiem tra authorization moi endpoint
- Secrets: khong hardcode passwords, API keys
- Input validation: whitelist > blacklist

## Go Review
- Error handling: khong ignore errors
- Goroutine leaks: moi goroutine co exit path
- Race conditions: shared state can mutex/channels
- Context: propagate ctx, respect deadlines
- Defer: close files, unlock mutexes, close rows
- Nil pointer: check nil truoc dereference

## TypeScript/React Review
- Strict mode, proper typing
- useEffect dependencies chinh xac
- Avoid unnecessary re-renders (memo, useMemo, useCallback)
- Error boundaries cho UI components
- Proper null/undefined handling

## Performance Red Flags
- Nested loops O(n^2) tren large datasets
- String concat trong loops
- Allocations trong hot paths (game loop)
- SELECT * thay vi specific columns
- Missing database indexes
