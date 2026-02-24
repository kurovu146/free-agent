# Git Workflow

## Conventional Commits
Format: `type(scope): description`

Types:
- `feat:` - tinh nang moi
- `fix:` - sua bug
- `docs:` - documentation
- `style:` - formatting, khong anh huong logic
- `refactor:` - refactor code
- `test:` - them/sua tests
- `chore:` - build, CI, dependencies
- `perf:` - cai thien performance

## PR Review Checklist
1. Code follow conventions? (gofmt, eslint)
2. Co test cho logic moi?
3. Security issues? (injection, XSS, hardcoded secrets)
4. Breaking changes?
5. Database migrations co rollback?
6. Error handling day du?
7. Naming conventions nhat quan?

## Branch Naming
- `feature/short-description`
- `fix/issue-description`
- `hotfix/critical-fix`
- `refactor/what-changed`

## Changelog Format
```
## [version] - YYYY-MM-DD
### Added
- feat commits
### Fixed
- fix commits
### Changed
- refactor commits
```
