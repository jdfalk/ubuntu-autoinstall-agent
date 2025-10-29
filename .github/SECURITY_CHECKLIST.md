<!-- file: .github/SECURITY_CHECKLIST.md -->
<!-- version: 1.0.0 -->
<!-- guid: a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d -->

# Workflow Security Audit Checklist

Complete this checklist before deploying new workflows or workflow changes.

## Date: ________________
## Reviewer: ________________
## Phase/PR: ________________

---

## 1. Secret Management

- [ ] No secrets hardcoded in workflow files
- [ ] All secrets use `${{ secrets.SECRET_NAME }}`
- [ ] Secrets referenced from GitHub Secrets, not environment variables
- [ ] No secret values appear in logs or outputs
- [ ] `::add-mask::` used where needed to prevent accidental exposure

**Notes:**

---

## 2. Permissions

- [ ] Workflow uses minimal required permissions
- [ ] No `permissions: write-all` or overly broad scopes
- [ ] Each job declares specific permissions needed
- [ ] Third-party actions granted minimal permissions
- [ ] Fork pull requests have restricted permissions

**Current Permissions:**

```yaml
permissions:
  contents: read
  pull-requests: write
```

---

## 3. Action Pinning

- [ ] All third-party actions pinned to commit SHA
- [ ] SHA documented with tag in comment (e.g., `# v4.1.1`)
- [ ] Dependabot configured to update pinned actions
- [ ] No mutable references (`@main`, `@v1`)

**Example:**

```yaml
- uses: actions/checkout@8e5e7e5ab8b370d6c329ec480221332ada57f0ab  # v4.1.1
```

---

## 4. Input Validation

- [ ] Workflow_dispatch inputs validated
- [ ] User inputs sanitized before execution
- [ ] No command injection vulnerabilities
- [ ] File paths validated (no directory traversal)
- [ ] Regular expressions tested for ReDoS issues

---

## 5. Code Execution

- [ ] No arbitrary code execution from PR comments
- [ ] `pull_request_target` avoided or carefully scoped
- [ ] Artifacts validated before extraction
- [ ] No execution of code from untrusted sources
- [ ] Scripts run with minimal privileges

---

## 6. Token Scoping

- [ ] `GITHUB_TOKEN` has appropriate scopes
- [ ] Personal access tokens avoided when `GITHUB_TOKEN` sufficient
- [ ] Token expiration configured
- [ ] Tokens not passed to untrusted actions
- [ ] API calls use authenticated endpoints

---

## 7. Dependency Security

- [ ] Python dependencies pinned with hashes
- [ ] Go modules validated with `go.sum`
- [ ] npm packages locked with `package-lock.json`
- [ ] Rust dependencies audited with `cargo audit`
- [ ] Dependabot enabled for updates

---

## 8. Environment Isolation

- [ ] No cross-contamination between jobs
- [ ] Temporary files cleaned up
- [ ] Secrets not persisted to disk
- [ ] Artifacts scanned before upload
- [ ] Containers use non-root users

---

## 9. Logging and Monitoring

- [ ] Sensitive data sanitized from logs
- [ ] Failed runs do not expose credentials
- [ ] Audit trail maintained for security events
- [ ] Anomaly detection configured
- [ ] Security incidents have response plan

---

## 10. Compliance

- [ ] Code follows `.github/instructions/security.instructions.md`
- [ ] SAST tools run on workflow code
- [ ] No compliance violations (SOC2, HIPAA, etc.)
- [ ] Security review completed
- [ ] Sign-off from security team

---

## Review Sign-off

- [ ] All items checked and passing
- [ ] Issues documented and tracked
- [ ] Deployment approved

**Reviewer Signature:** ________________

**Date:** ________________

**Notes:**
