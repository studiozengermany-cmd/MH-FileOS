# PLANS.md — Execution Plan Standard

## 1. Mục đích

Execution Plan là tài liệu sống cho công việc nhiều bước. Nó giúp một agent mới tiếp tục được dự án mà không phải đoán ý, đồng thời tạo checkpoint để chủ sản phẩm kiểm tra trước khi phạm vi lan rộng.

Plan phải tự chứa đủ thông tin để thực thi. Không viết kiểu “làm như đã trao đổi” hoặc phụ thuộc vào hội thoại không nằm trong repository.

## 2. Khi nào bắt buộc có Execution Plan

- Task kéo dài qua nhiều phiên.
- Thay đổi từ hai module trở lên.
- Thêm dependency, migration hoặc public API.
- Chạm scanner, planner, executor, journal, undo, recovery.
- Có rủi ro dữ liệu, hiệu năng hoặc compatibility.
- Cần chia milestone/checkpoint.

Task tài liệu nhỏ hoặc bug rõ ràng một file có thể dùng kế hoạch ngắn trong phiên, trừ khi ảnh hưởng safety invariant.

## 3. Quy tắc viết Plan

Mỗi plan phải có:

1. ID và trạng thái.
2. Outcome người dùng nhận được.
3. Context và vấn đề.
4. Scope.
5. Non-scope.
6. Safety impact.
7. Milestones theo thứ tự.
8. Acceptance criteria.
9. Verification commands/strategy.
10. Recovery/rollback.
11. Decision log.
12. Progress log có ngày.

Không liệt kê task mơ hồ như “làm backend” hoặc “hoàn thiện UI”. Mỗi milestone phải tạo ra một bằng chứng quan sát được.

## 4. Trạng thái chuẩn

- `DRAFT`
- `APPROVED`
- `IN_PROGRESS`
- `BLOCKED`
- `VERIFYING`
- `COMPLETED`
- `SUPERSEDED`

Chỉ chủ sản phẩm hoặc yêu cầu rõ ràng mới chuyển từ DRAFT sang APPROVED đối với plan có mutation hoặc dependency mới.

## 5. Mẫu Execution Plan

```md
# EP-XYZ — Tên outcome

Status: DRAFT
Owner: ...
Created: YYYY-MM-DD
Last updated: YYYY-MM-DD

## Outcome
...

## Context
...

## Scope
- ...

## Non-scope
- ...

## Safety impact
- Invariants liên quan: SI-...
- Dữ liệu có thể bị ảnh hưởng: ...
- Recovery: ...

## Milestones

### M0 — ...
Deliverables:
- ...
Acceptance:
- ...
Verification:
- ...

## Rollback/Recovery
...

## Decisions
- YYYY-MM-DD — Quyết định — Lý do

## Progress
- [ ] YYYY-MM-DD — ...

## Evidence
- Command/test/report: ...

## Remaining risks
- ...
```

---

# EP-000 — Repository Bootstrap và Read-only Foundation

**Status:** VERIFYING — M0–M6 local checkpoints complete; Windows CI and EP-000 safety acceptance pending

- **Owner:** Primary implementation agent
- **Created:** 2026-07-13
- **Last updated:** 2026-07-27

## Outcome

Tạo nền móng repository có thể build/test lặp lại và một vertical slice hoàn toàn read-only:

```text
Fixture sandbox → Scanner CLI → SQLite Catalog → Query summary
```

Kết thúc EP-000, dự án phải chứng minh được rằng nó có thể quét một fixture do test tạo, ghi metadata vào catalog và xuất summary mà không thay đổi file nguồn.

EP-000 không tạo UI sản phẩm hoàn chỉnh và không thực hiện move/rename/delete.

## Context

MH FileOS là công cụ quản lý file có rủi ro dữ liệu cao. Bắt đầu bằng dashboard hoặc automation sẽ che giấu việc core chưa có bằng chứng an toàn. EP-000 dựng test harness, module boundary và read-only pipeline trước.

## Scope

- Git repository hygiene.
- Rust workspace skeleton.
- Tauri/React shell chỉ khi core CLI pass.
- SQLite migration v1 cho volume/root/scan/file metadata tối thiểu.
- Fixture generator deterministic.
- Scanner recursive read-only, bounded queue, cancellation.
- Catalog batch writer.
- CLI summary và machine-readable report.
- CI format/lint/test/build.
- Tài liệu command thực tế sau khi toolchain được chốt.

## Non-scope

- Move, rename, delete, quarantine.
- Duplicate full hashing.
- Rule engine.
- Planner/executor/journal.
- Watcher.
- Auto Mode.
- Production installer.
- Whole-drive onboarding UX, approved-root registry hoặc cờ opt-in riêng; production CLI hiện nhận một absolute directory do người dùng chọn.
- Automated verification trên dữ liệu cá nhân; mọi test scan vẫn dùng synthetic fixture hoặc test temp.
- AI/API/cloud.

## Safety impact

Invariants áp dụng:

- SI-001 Read-only means read-only.
- SI-002 Scope confinement.
- SI-010 No shell command interpolation.
- SI-013 Test isolation.
- SI-017 Truthful result reporting.

Mọi automated scan test chỉ được chạy trong `fixtures/sandbox` hoặc OS temp directory do `fileos-testkit` tạo. Production CLI được nhận một absolute directory do người dùng chủ động chọn, nhưng vẫn chỉ quan sát, giữ resource bounds và không theo reparse target.

## Milestone 0 — Repository Contract Verification

### Deliverables

- Các file tài liệu gốc tồn tại và liên kết đúng.
- Chưa có production dependency.
- Git ignore policy được xác định khi repo được init.
- Toolchain decision được ghi trong ADR đầu tiên trước khi pin version.

### Acceptance

- Agent tóm tắt đúng năm safety invariant liên quan EP-000.
- Agent xác nhận không quét ngoài fixture.
- Không file production giả được tạo.

### Verification

- Liệt kê tree.
- Kiểm tra link Markdown nội bộ.
- Kiểm tra dung lượng `AGENTS.md` dưới giới hạn context mặc định hợp lý.

### Checkpoint

Dừng để chủ sản phẩm duyệt trước khi cài toolchain hoặc tạo workspace manifest.

## Milestone 1 — Toolchain Baseline

### Deliverables

- Rust stable pin có lý do.
- Node LTS pin có lý do.
- Package manager được chốt.
- CI Windows chạy format/lint/test rỗng tối thiểu.
- Không auto-download binary ngoài package manager chuẩn.

### Acceptance

- Cùng command chạy được local và CI.
- Version được ghi trong file toolchain/manifest, không chỉ README.
- Không có warning bị bỏ qua.

### Verification dự kiến

- `rustc --version`
- `cargo --version`
- Node/package manager version.
- CI workflow validation.

### Checkpoint

Dừng và báo dependency/toolchain đã thêm.

## Milestone 2 — Rust Workspace và Module Contracts

### Deliverables

Workspace tối thiểu:

```text
crates/
├─ fileos-domain/
├─ fileos-app/
├─ fileos-testkit/
├─ fileos-catalog/
├─ fileos-scanner/
└─ fileos-platform-windows/
```

Milestone 2 chỉ thiết lập workspace và responsibility/module contracts biên dịch được; không triển khai filesystem, SQLite hoặc mutation API và không thêm external production dependency.

Không tạo planner/executor trước khi EP-000 hoàn thành.

### Acceptance

- Workspace có đủ sáu crate boundary và không có dependency cycle.
- Domain và application crates biên dịch độc lập với Tauri, React, SQLite và concrete scanner/catalog/platform adapter.
- Scanner, catalog và Windows adapter chỉ có responsibility contract; không có filesystem, SQLite hoặc mutation implementation.
- `cargo metadata --no-deps` chỉ chứa sáu workspace crate đã duyệt và không có external production dependency.
- `cargo test --workspace` chạy.
- Không production API giả; crate contract chỉ chứa responsibility documentation và lint boundary cho tới milestone triển khai hành vi tương ứng.

### Verification

- Dependency graph review.
- `cargo fmt --all -- --check`.
- Workspace tests.
- `cargo clippy --workspace --all-targets -- -D warnings`.

### Checkpoint

Dừng và bàn giao bằng chứng workspace local trước khi bắt đầu fixture generator.

## Milestone 3 — Deterministic Fixture Generator

**Approval:** Product owner approved implementation on 2026-07-14.

### Implementation contract

- `fileos-testkit` remains standard-library-only; no production dependency is added.
- Repository fixtures are created only below `fixtures/sandbox/runtime/<run-id>/`.
- Callers receive capability types rather than unrestricted cleanup functions over arbitrary paths.
- Marker and manifest identity, canonical containment, repository identity and reparse-point checks fail closed before cleanup.
- Cleanup walks entries without following reparse points and consumes the run capability so a successful cleanup cannot be repeated accidentally.
- Fixture content and logical manifest are deterministic for a seed; the unique runtime directory identity is intentionally excluded from logical equality.
- The fixture digest is versioned and used only for deterministic test evidence, never as production duplicate proof.

### Deliverables

Fixture có thể tạo:

- File rỗng.
- File nhỏ/lớn giả lập hợp lý.
- Nested directories.
- Unicode, tiếng Việt, emoji.
- Khoảng trắng, apostrophe, ampersand.
- File read-only.
- Symlink/junction nếu môi trường cho phép, luôn nằm trong sandbox.
- Permission-denied scenario nếu có thể mô phỏng an toàn.

Fixture manifest lưu expected paths, sizes và hashes cho file cần thiết.

### Acceptance

- Cùng seed tạo cùng logical fixture.
- Cleanup không thể thoát sandbox.
- Test chứng minh cleanup từ chối path ngoài sandbox.

### Verification

- Unit/property tests.
- Tree snapshot.
- Scope escape negative tests.
- Marker/manifest tamper negative tests.
- `scripts/verify.ps1` local aggregate gate.

## Milestone 4 — Read-only Scanner CLI

**Approval:** Product owner approved implementation and isolated multi-agent task division on 2026-07-14.

### Implementation contract

- M4 remains standard-library-only except for explicit in-workspace dependencies; no external production dependency is added.
- Domain/application crates own typed scan lifecycle, counters, issues, cancellation and ports; scanner/platform crates own read-only observation implementations; the CLI is composition only.
- The scanner streams observations to a sink and never retains the full scan result. Every work queue, error sample and open-directory count is configured, bounded and reported with high-water evidence.
- `Completed` means traversal exhausted with zero partial issue. Inaccessible/disappeared entries produce `CompletedWithIssues`; observed cancellation produces `Cancelled`; invalid/inaccessible roots remain fatal typed failures.
- Root validation is explicit. Reparse points are observed/skipped without resolving or traversing their targets; M4 does not claim adversarial reparse-swap protection from path-based standard-library APIs.
- Deterministic JSON summary v1 contains stable counters, grouped issue codes, configured limits and measured high-water values only; it excludes absolute roots, filenames, timestamps, duration, run IDs and OS error messages.
- Verification scans only `SandboxRun::scan_root()`. Before/after evidence covers paths, entry kinds, content/size, modified time and attributes where observable; access-time noise is reported separately rather than broadly ignored.
- File identity, real Windows ACL denial, hostile reparse TOCTOU and million-entry hardware benchmarks remain deferred evidence, not inferred M4 capabilities.

### Deliverables

- Recursive enumeration.
- Metadata collection.
- Bounded work queue.
- Cancellation.
- Error accumulation.
- Skip reparse points mặc định.
- Progress aggregate.
- JSON summary.

### Acceptance

- Không thay đổi content/path/size/mtime của fixture.
- Không crash khi file biến mất giữa scan.
- Không fail toàn scan vì một file permission denied.
- Cancel kết thúc có kiểm soát.
- Scanner không theo link ra ngoài root.

### Verification

- Before/after fixture manifest comparison.
- Integration tests.
- Cancellation test.
- Reparse escape test.

## Milestone 5 — SQLite Catalog v1

**Approval:** Product owner approved implementation and the required SQLite dependency decision on 2026-07-14.

### Implementation contract

- `fileos-catalog` pins `rusqlite 0.40.1` with only the `bundled` feature; the dependency, alternatives, license, size/security/maintenance impact and rollback are recorded in `docs/adr/ADR-002-sqlite-catalog-v1.md`.
- Schema v1 owns `volumes`, `scan_roots`, `scan_runs`, and `file_entries`; migrations are atomic and newer schemas fail closed.
- Every connection enables foreign keys and an explicit busy timeout. WAL is used only after verifying SQLite accepted it.
- A bounded in-memory batch is written in a short transaction; scanner filesystem metadata I/O never occurs while a database transaction is open.
- Entry identity is unique within a scan root by opaque path key. Repeat observations update one current row while retaining first-seen, last-seen, and missing-since scan evidence.
- Only a fully `completed` authoritative scan may mark unseen rows missing. Cancelled, failed, and completed-with-issues runs never infer absence.
- M5 category summary uses the stable observed entry kind (`file`, `directory`, `reparse_point`, `other`). Extension/content classification remains outside M5.
- Integration databases live below a testkit sandbox run's `artifacts` directory; fixture `input` is snapshotted before and after scanner-to-catalog tests.
- Catalog errors expose stable codes, retryability, safe messages, and migration version where relevant; no filesystem path or raw SQL is included in user-safe output.

### Deliverables

- Migration v1.
- Tables tối thiểu: volumes, scan_roots, scan_runs, file_entries.
- Batch insert/update.
- Query summary theo category/size/root.
- Temp DB integration tests.

### Acceptance

- Catalog interface được kiểm thử bằng temp DB do test tạo trong scope riêng.
- Chạy lại scan cập nhật entry, không nhân bản vô hạn.
- File mất được đánh trạng thái, không xóa history mù.
- DB error trả error typed.
- Không giữ DB lock trong metadata I/O.

### Verification

- Fresh migration.
- Reopen DB.
- Repeat scan.
- Corruption/error simulation ở mức hợp lý.
- Foreign-key and newer-schema rejection tests.
- Scanner-to-catalog integration with before/after fixture proof.

## Milestone 6 — Read-only Vertical Slice

**Approval:** Product owner approved implementation on 2026-07-14.

### Implementation contract

- The single internal demo command is a Cargo example owned by `fileos-cli`; it uses `fileos-testkit` and `fileos-catalog` only as dev-dependencies and does not change the production CLI command surface.
- The command creates one deterministic fixture below `fixtures/sandbox/runtime`, stores SQLite only below that run's `artifacts` directory, then performs `snapshot → scan → catalog → summary → snapshot comparison → capability cleanup`.
- The synthetic fixture run ID is the explicit volume identity for this internal workflow; M6 does not infer a production volume identity from a drive letter or path.
- The final JSON is deterministic for the fixed seed plus recorded host capability outcome and is path-free. It reports only schema/status, catalog counts/bytes and boolean verification evidence; raw paths, filenames, run IDs, timestamps and SQLite messages are excluded.
- Only a fully completed scan is accepted by the demo. Any partial/cancelled/failed scan, summary mismatch, source snapshot mismatch, output failure or cleanup failure returns a typed path-free demo error and never claims success.
- Scanner and catalog retain their existing bounds; no filesystem I/O occurs while a catalog transaction is held. No new external dependency, migration, UI, move, rename or delete capability is introduced.
- Verification uses a fixed synthetic seed and checks runtime residue. Rollback removes the example target, its two dev-dependency/inventory entries and M6-only evidence without touching the M5 catalog schema.

### Deliverables

Command duy nhất cho demo nội bộ:

```text
generate fixture → scan fixture → catalog → print summary → verify unchanged
```

### Acceptance

- Chạy một command/test suite có bằng chứng cuối.
- Báo số file/thư mục/dung lượng đúng fixture manifest.
- Xuất report không chứa path ngoài fixture.
- Không mutation source.

### Verification

- Full workspace test.
- Clippy/format.
- Diff review.
- Evidence report lưu trong CI artifact hoặc test output, không commit runtime DB.

### Checkpoint

Dừng implementation tại M6 local checkpoint. EP-000 chỉ được đóng sau Windows CI và safety acceptance; không tự chuyển sang UI hoặc duplicate engine.

## Milestone 7 — Optional Desktop Shell Spike

Milestone này chỉ được duyệt sau Milestone 6.

### Scope

- Tauri window đọc summary từ core command.
- Không file table lớn.
- Không fake dashboard.
- Không mutation command.

### Acceptance

- Số liệu UI bằng dữ liệu fixture thật.
- Core vẫn test độc lập UI.
- CSP không để null.
- Capability tối thiểu.

## Rollback/Recovery

- EP-000 không mutation dữ liệu người dùng; production CLI chỉ quan sát root do người dùng chọn.
- Mỗi milestone là commit riêng.
- Nếu toolchain/dependency không phù hợp, revert milestone đó thay vì chắp vá.
- Temp DB và fixture có cleanup giới hạn scope.
- Không dùng `git clean/reset --hard` làm rollback tự động.

## Decisions

- 2026-07-13 — Windows 10/11 first — Core cần semantics NTFS và Windows File ID.
- 2026-07-13 — Rust core + SQLite — Phù hợp I/O, state machine và catalog local.
- 2026-07-13 — Read-only vertical slice trước UI — Giảm rủi ro fake functionality.
- 2026-07-13 — No WSL requirement cho Windows integration tests — Tránh khác semantics filesystem.
- 2026-07-13 — No production dependency installation trong Milestone 0 — Chủ sản phẩm phải duyệt baseline.
- 2026-07-14 — Local development trước GitHub/CI — Toolchain được pin và kiểm tra local; không push, PR hoặc workflow khi chưa có yêu cầu mới.
- 2026-07-14 — ADR-000 chốt Rust `1.97.0`, Node `24.17.0` LTS và pnpm `10.11.0` — Dùng đúng tool đã có trên máy, không auto-install.
- 2026-07-14 — M2 dùng sáu crate contract gồm `fileos-app`; temp-DB acceptance chuyển sang M5 — Đồng bộ dependency order với kiến trúc và không triển khai SQLite trước milestone catalog.
- 2026-07-14 — Chủ sản phẩm duyệt M3 cùng authority cleanup — Fixture generator dùng capability type, standard library và fail-closed cleanup; không khôi phục test trên dữ liệu người dùng thật.
- 2026-07-14 — Chủ sản phẩm duyệt M4 và chia việc bằng worktree riêng — Scanner chỉ đọc fixture trong verification; không thêm dependency, catalog, UI hoặc mutation ngoài phạm vi đã duyệt.
- 2026-07-14 — Chủ sản phẩm duyệt M5 SQLite Catalog v1 — Pin `rusqlite 0.40.1` với bundled SQLite, triển khai migration/upsert/conservative absence/typed errors trong sandbox; UI và mọi move/delete vẫn ngoài phạm vi.
- 2026-07-27 — M6 local checkpoint hoàn tất; EP-000 chuyển sang `VERIFYING` cho Windows CI và safety acceptance, chưa chuyển sang M7.
- 2026-07-27 — Production CLI giữ hành vi scan read-only một absolute directory mặc định; fixture-only áp dụng cho automated verification, không thêm allow flag hoặc approved-root registry.
- 2026-07-27 — Lát cắt CI bắt đầu từ canonical GitHub `main`; lịch sử local M0–M6 độc lập được giữ để tham chiếu, không merge unrelated histories.

## Progress

- [x] 2026-07-13 — Hoàn thành bộ Repository Contract.
- [x] 2026-07-14 — Chủ sản phẩm giao thực hiện riêng Milestone 0; chưa duyệt bắt đầu Milestone 1 hoặc cài dependency.
- [x] 2026-07-14 — Milestone 0 verification hoàn tất; dừng tại checkpoint trước Toolchain Baseline.
- [x] 2026-07-14 — Chủ sản phẩm duyệt bắt đầu Milestone 1 theo hướng local-first.
- [x] 2026-07-14 — Milestone 1 local toolchain pins và verifier pass; không cài hoặc tải dependency/tool mới.
- [x] 2026-07-27 — Windows CI workflow đã được thêm trên branch canonical và dùng aggregate verifier hiện hữu.
- [ ] Milestone 1 Windows CI validation — chờ workflow chạy thành công trên GitHub tại đúng commit SHA.
- [ ] Milestone 1 full acceptance — Windows CI validation còn thiếu và phải hoàn tất trước khi đóng EP-000.
- [x] 2026-07-14 — Chủ sản phẩm duyệt bắt đầu Milestone 2 và phạm vi điều chỉnh Execution Plan.
- [x] 2026-07-14 — Milestone 2 local workspace contracts hoàn tất; dừng tại checkpoint trước fixture generator.
- [x] 2026-07-14 — Chủ sản phẩm duyệt bắt đầu Milestone 3 và authority cleanup.
- [x] 2026-07-14 — Milestone 3 deterministic fixture generator hoàn tất local checkpoint; dừng trước scanner Milestone 4.
- [x] 2026-07-14 — Milestone 4 read-only scanner CLI hoàn tất local checkpoint; dừng trước catalog Milestone 5.
- [x] 2026-07-14 — Milestone 5 SQLite Catalog v1 hoàn tất local checkpoint sau remediation và final independent re-review; dừng trước Milestone 6.
- [x] 2026-07-14 — Milestone 6 read-only vertical slice hoàn tất local checkpoint bằng single-command fixture demo; dừng trước UI/Milestone 7 tùy chọn.
- [ ] EP-000 safety traceability review/sign-off — cần phân loại hoặc chấp nhận rõ các residual evidence limits trước khi đóng plan.

## Evidence

### Milestone 0 — 2026-07-14

- **[ĐÃ XÁC MINH BẰNG TEST]** `rg --files` và inventory đệ quy xác nhận repository hiện chỉ có tài liệu, policy và README giữ chỗ; chưa có workspace manifest, package manifest, production code, symlink, junction hoặc reparse point.
- **[ĐÃ XÁC MINH BẰNG TEST]** Trình kiểm tra link Markdown tương đối trên toàn bộ `*.md` báo tất cả target đều tồn tại; repository không có link Markdown chứa fragment cần kiểm tra anchor.
- **[ĐÃ XÁC MINH BẰNG TEST]** SHA-256 trên toàn bộ file không phát hiện file byte-identical. Có một tài liệu trùng vai trò về mặt ngữ nghĩa: `MH-FILEOS-MASTER-PLAN-MVP-v1.0.md` nằm ngoài tree chuẩn và song song với `docs/MASTER-PLAN.md`.
- **[ĐÃ XÁC MINH BẰNG FILE]** `README.md` không liệt kê file master plan ở root trong cấu trúc khởi đầu; file này cũng không xuất hiện trong thứ tự nguồn sự thật của `AGENTS.md`.
- **[ĐÃ XÁC MINH BẰNG FILE]** Có hai điểm cần xử lý bằng quyết định tài liệu: thứ tự đọc trong `README.md` đặt `AGENTS.md` trước `docs/SAFETY-INVARIANTS.md`, khác thứ tự authority bắt buộc; bản master plan ở root cho phép một vòng test bằng bản sao dữ liệu thật đã backup, không phù hợp test isolation hiện hành chỉ dùng synthetic fixture/test temp.
- **[ĐÃ XÁC MINH BẰNG TEST]** Toolchain hiện có: Git `2.54.0.windows.1`, Rust `1.97.0`, Cargo `1.97.0`, Node.js `v24.17.0`, npm `11.13.0`, pnpm `10.11.0`. Không cài hoặc cập nhật dependency.
- **[ĐÃ XÁC MINH BẰNG TEST]** `AGENTS.md` có kích thước 12,277 byte; **[SUY LUẬN]** phù hợp để nạp nguyên văn trong context mặc định hiện tại.
- **[ĐÃ XÁC MINH BẰNG FILE]** Năm invariant áp dụng cho EP-000 được hiểu như sau: SI-001 cấm scan/catalog/analysis làm thay đổi file; SI-002 giới hạn mọi write trong scope đã duyệt và chặn reparse escape; SI-010 cấm nội suy path qua shell; SI-013 chỉ cho test mutation trong sandbox có containment/identity hợp lệ; SI-017 yêu cầu nhãn bằng chứng trung thực.
- **[ĐÃ XÁC MINH BẰNG FILE]** Milestone 0 không quét ngoài repository/fixture, không tạo production code và không chạm file người dùng. `.gitignore` đã loại trừ Rust/Node build output, fixture runtime, database/journal/log, secret và package artifact.
- **[CHƯA KIỂM THỬ]** Chưa có build, lint, unit/integration test, fixture before/after proof hoặc CI run vì Milestone 0 chưa có production code hay manifest.

### Milestone 1 local baseline — 2026-07-14

- **[ĐÃ XÁC MINH BẰNG FILE]** `docs/adr/ADR-000-toolchain-baseline.md` ghi context, quyết định, alternatives, safety impact, compatibility và rollback trước khi pin toolchain.
- **[ĐÃ XÁC MINH BẰNG FILE]** `rust-toolchain.toml`, `.node-version` và `package.json` pin Rust `1.97.0`, Node `24.17.0` LTS và pnpm `10.11.0`; root package là private và không có dependency hoặc script.
- **[ĐÃ XÁC MINH BẰNG TEST]** `pwsh -NoProfile -File scripts/verify-toolchain.ps1` pass với rustc `1.97.0`, Cargo `1.97.0`, rustfmt `1.9.0-stable`, Clippy `0.1.97`, Node `v24.17.0` và pnpm `10.11.0`.
- **[ĐÃ XÁC MINH BẰNG TEST]** Verifier đọc và đối chiếu đồng thời file pin, root manifest và version runtime; Rust component được gọi qua exact installed toolchain bằng `rustup run`, không chạy lệnh update/install.
- **[CHƯA KIỂM THỬ]** Chưa có Windows CI run theo chỉ đạo local-first. Chưa có Cargo workspace, TypeScript project, build, unit test hoặc integration test vì các artifact đó thuộc milestone sau.

### Milestone 2 local workspace contracts — 2026-07-14

- **[ĐÃ XÁC MINH BẰNG FILE]** Root `Cargo.toml` khai báo đúng sáu boundary theo thứ tự kiến trúc: `fileos-domain`, `fileos-app`, `fileos-testkit`, `fileos-catalog`, `fileos-scanner`, `fileos-platform-windows`; resolver `3`, edition `2024`, Rust `1.97.0` và `unsafe_code = "forbid"` được đặt ở workspace.
- **[ĐÃ XÁC MINH BẰNG FILE]** Mỗi crate chỉ có manifest và crate documentation mô tả trách nhiệm; chưa có production API, filesystem/SQLite implementation, mutation capability, test giả hoặc external dependency. `crates/README.md` đã được đồng bộ để không còn mô tả thư mục crate là trống trong Milestone 2.
- **[ĐÃ XÁC MINH BẰNG TEST]** `scripts/verify-workspace.ps1` pass: xác minh toolchain pin, `cargo fmt --all -- --check`, metadata đúng sáu package, zero dependency, đúng một lib target mỗi crate, dependency tree, workspace check, Clippy với `-D warnings`, workspace test và rustdoc với `-D warnings`; toàn bộ lệnh Cargo dùng `--locked --offline` khi áp dụng.
- **[ĐÃ XÁC MINH BẰNG TEST]** Clippy `0.1.97` pass khi verifier gọi trực tiếp `cargo-clippy` của toolchain `1.97.0-x86_64-pc-windows-msvc`; đường Cargo proxy `cargo clippy` trên máy hiện tại bị treo và để lại process chờ, nên các process do phiên kiểm định tạo đã được nhận diện theo PID/command line và dừng trước khi chạy lại bằng binary đã pin.
- **[ĐÃ XÁC MINH BẰNG TEST]** `cargo test --workspace --all-targets --all-features --locked --offline` pass với 0 test và 0 failure, phù hợp vì Milestone 2 chỉ có contract documentation, chưa có hành vi nghiệp vụ.
- **[CHƯA KIỂM THỬ]** Chưa có behavioral, filesystem, SQLite, Windows reparse/permission, fixture integration, cancellation, performance hoặc CI evidence; các bằng chứng này thuộc milestone triển khai tương ứng và không được suy diễn từ workspace compile.

### Milestone 3 deterministic fixture generator — 2026-07-14

- **[ĐÃ XÁC MINH BẰNG FILE]** `fileos-testkit` chỉ public capability types `SandboxBase`/`SandboxRun`; không có cleanup API nhận arbitrary path. Mọi run nằm trực tiếp dưới canonical `fixtures/sandbox/runtime/<run-id>/` và cần repository identity, marker cùng manifest khớp trước cleanup.
- **[ĐÃ XÁC MINH BẰNG FILE]** Fixture manifest có seed, schema/generator/digest version, ordered portable paths, size/digest/read-only evidence và optional scenarios. `fnv1a64-fixture-v1` được ghi rõ chỉ là deterministic fixture evidence, không phải production duplicate proof.
- **[ĐÃ XÁC MINH BẰNG TEST]** `cargo test -p fileos-testkit --all-targets --locked --offline` pass 6 unit tests và 7 integration tests: bounded seed determinism, required tree cases, scope rejection, repository identity, marker/manifest tamper, missing marker, read-only cleanup, internal symlink và outside-target symlink không bị follow.
- **[ĐÃ XÁC MINH BẰNG TEST]** `scripts/verify.ps1` pass toolchain/workspace metadata, format, check, Clippy `-D warnings`, all workspace tests, rustdoc `-D warnings`, Markdown links, Rust-manifest JSON parse qua `ConvertFrom-Json`, zero fixture runtime residue và `git diff --check`.
- **[ĐÃ XÁC MINH BẰNG TEST]** Cargo metadata vẫn có đúng sáu workspace package, zero external source và zero dependency; năm crate ngoài testkit vẫn docs-only.
- **[ĐÃ XÁC MINH BẰNG TEST]** Windows host hiện tại tạo được symlink fixture; cleanup dùng `std::fs::remove_dir_all` của pinned Rust sau scope/identity validation và test chứng minh file target bên ngoài sandbox giữ nguyên byte.
- **[CHƯA KIỂM THỬ]** Chưa mô phỏng permission-denied bằng ACL, chưa có API reopen/recover orphaned fixture capability của testkit sau process crash và chưa có Windows CI evidence. Các giới hạn này được ghi trong `docs/SAFETY-TRACEABILITY.md`.

### Milestone 4 read-only scanner CLI — 2026-07-14

- **[ĐÃ XÁC MINH BẰNG FILE]** `fileos-domain` và `fileos-app` sở hữu lifecycle, counters, typed issues, cancellation, resource limits và scan ports; `fileos-scanner`/`fileos-platform-windows` chỉ quan sát; `apps/fileos-cli` chỉ composition và không có mutation API.
- **[ĐÃ XÁC MINH BẰNG FILE]** JSON summary schema v1 chỉ chứa status, stable counters, grouped issue codes, configured limits và measured high-water values; không chứa root, path, filename, timestamp, duration, run ID hoặc raw OS error.
- **[ĐÃ XÁC MINH BẰNG TEST]** `cargo test -p fileos-scanner --all-targets --locked --offline` pass 7/7: before/after read-only proof, deterministic summary, pre/mid cancellation, bounded open-directory partial result, invalid-limit fail-before-I/O, disappearing-file continuation, outside-target reparse no-follow và typed permission mapping.
- **[ĐÃ XÁC MINH BẰNG TEST]** `cargo test -p fileos-testkit --all-targets --locked --offline` pass 6 unit + 10 integration tests; snapshot oracle phát hiện thay đổi content cùng kích thước và áp dụng giới hạn entry/depth/file-size.
- **[ĐÃ XÁC MINH BẰNG TEST]** `cargo test -p fileos-cli --all-targets --locked --offline` pass 2/2; CLI quét fixture bằng absolute root, xuất JSON không có runtime path/tên fixture và giữ snapshot trước–sau bằng nhau.
- **[ĐÃ XÁC MINH BẰNG TEST]** `scripts/verify-workspace.ps1` pass exact inventory/dependency/target gates cho 7 workspace package, format, check, Clippy `-D warnings`, toàn bộ test và rustdoc `-D warnings`; Cargo chạy locked/offline và không có external dependency.
- **[ĐÃ XÁC MINH BẰNG TEST]** `pwsh -NoProfile -File scripts/verify.ps1` pass aggregate local gate trong 37,1 giây: workspace/toolchain/tests, Markdown links, fixture-manifest JSON, path-free scan-summary JSON, zero fixture runtime residue và `git diff --check`.
- **[CHƯA KIỂM THỬ]** Tại checkpoint Milestone 4, real Windows ACL denial, hostile reparse-swap TOCTOU, file identity, non-Windows runtime, million-entry performance và CI chưa có runtime evidence; bằng chứng catalog hiện hành nằm trong mục Milestone 5 bên dưới.

### Milestone 5 SQLite Catalog v1 — 2026-07-14

- **[ĐÃ XÁC MINH BẰNG FILE]** ADR-002 chốt `rusqlite 0.40.1`, `libsqlite3-sys 0.38.1`, bundled SQLite `3.53.2`, MIT/public-domain, alternatives, dependency impact và rollback; chỉ feature `bundled` được bật.
- **[ĐÃ XÁC MINH BẰNG FILE]** Migration v1 có `volumes`, `scan_roots`, `scan_runs`, `file_entries`, foreign keys `RESTRICT`, partial unique active-run index, root/path upsert identity và present/missing lifecycle; không có API mutation user file.
- **[ĐÃ XÁC MINH BẰNG TEST]** `cargo test -p fileos-catalog --all-targets --locked --offline` pass 1 unit + 15 integration tests: fresh migration/reopen/settings, repeat convergence/read-only proof, conservative missing/reappear behavior, newer/foreign/corrupt DB, busy writer, direct partial-unique active-run mapping, typed sink-error terminalization, explicit interrupted-run recovery, report counter/issue/durable-count binding, exact-schema counterfeit-index rejection và persisted foreign-key rejection.
- **[ĐÃ XÁC MINH BẰNG TEST]** `scripts/verify-workspace.ps1` pass trong 21,6 giây: format, exact dependency/file/target inventory, Cargo tree/check, Clippy `-D warnings`, full workspace tests và rustdoc với locked/offline dependencies.
- **[ĐÃ XÁC MINH BẰNG TEST]** Sau phê duyệt cleanup riêng, 7 run synthetic từ lần test đỏ (seed 5001–5004, 5006–5008) được kiểm tra lại canonical containment, direct-child scope, marker schema/run ID/disposable/seed, manifest và reparse state trước khi xóa đúng 7 target; runtime sandbox còn 0 item.
- **[ĐÃ XÁC MINH BẰNG TEST]** `pwsh -NoProfile -File scripts/verify.ps1` pass aggregate local gate; final post-status rerun hoàn tất trong 17,7 giây: workspace/toolchain/dependency inventory, format, check, Clippy `-D warnings`, toàn bộ test, rustdoc, Markdown links, manifest/scan JSON probes, zero fixture runtime residue và `git diff --check`.
- **[CHƯA KIỂM THỬ]** Chưa có child-process kill giữa migration, immutable golden upgrade từ schema cũ, million-entry benchmark, concurrent-reader soak, Windows volume/file identity, Windows 10/11 CI hoặc non-Windows runtime evidence.
- **[ĐÃ XÁC MINH BẰNG TEST]** Final independent re-review không còn finding P0–P2 sau remediation: catalog có explicit recovery/terminalization, exact-schema comparison, foreign-key check, counter/issue/durable report binding và direct unique-constraint mapping. Ghi chú P3 về cách gọi orphaned fixture capability đã được làm rõ trước handoff.
- **[ĐÃ XÁC MINH BẰNG TEST]** Final runtime check và aggregate verifier đều xác nhận `fixtures/sandbox/runtime/` còn 0 item; không database runtime, fixture residue hoặc dữ liệu người dùng được đưa vào repository.

### Milestone 6 read-only vertical slice — 2026-07-14

- **[ĐÃ XÁC MINH BẰNG FILE]** `apps/fileos-cli/examples/fixture_catalog_demo.rs` là dev-only Cargo example; production `fileos-cli` command không đổi. Workflow dùng fixed seed 6001, synthetic run ID làm explicit test volume identity, SQLite dưới capability-owned `artifacts` và không thêm external dependency hay filesystem mutation API.
- **[ĐÃ XÁC MINH BẰNG TEST]** `cargo run --quiet -p fileos-cli --example fixture-catalog-demo --locked --offline` pass một command end-to-end: 10 entries, 6 file, 4 thư mục, 65.930 bytes trên host hiện tại; catalog reopen/integrity pass, manifest = snapshot = catalog summary, source trước–sau không đổi, sandbox cleanup thành công và JSON không chứa path/run ID.
- **[ĐÃ XÁC MINH BẰNG TEST]** `cargo test -p fileos-cli --all-targets --locked --offline` pass 4/4: hai M4 production CLI tests và hai M6 example tests, gồm success evidence và typed output failure sau safe cleanup.
- **[ĐÃ XÁC MINH BẰNG TEST]** `scripts/verify-workspace.ps1` pass trong 20,3 giây: exact target/file/dependency inventory, format, check, Clippy `-D warnings`, toàn bộ workspace tests và rustdoc với locked/offline dependencies.
- **[ĐÃ XÁC MINH BẰNG TEST]** `pwsh -NoProfile -File scripts/verify.ps1` pass aggregate local gate; final post-status rerun hoàn tất trong 19,7 giây: M6 command JSON được parse/validate, existing manifest/scan probes pass, Markdown links và Git diff sạch, runtime sandbox còn 0 item.
- **[ĐÃ XÁC MINH BẰNG FILE]** Diff review xác nhận chỉ có M6 example/dev-dependency/verifier/docs; không migration mới, UI, network, move, rename, delete hoặc user-file write path.
- **[CHƯA KIỂM THỬ]** Process-kill giữa fixture demo có thể để lại orphaned fixture capability cần cleanup được duyệt; optional symlink không được tạo trên host run này; Windows 10/11 CI, non-Windows runtime, performance/soak và production volume identity composition vẫn chưa có bằng chứng M6.

### Evidence còn thiếu để đóng EP-000

- Windows CI run thành công tại đúng commit SHA, kèm run URL/log aggregate verifier.
- Safety traceability review/sign-off cho các invariant áp dụng và residual evidence limits.
- Quyết định rõ blocker nào phải xử lý trong EP-000, giới hạn nào được defer sang hardening sau đó.

## Remaining risks

- Node `24.17.0` là LTS và đã có local nhưng không phải patch Node 24 mới nhất tại ngày quyết định; nâng patch cần thay đổi có chủ đích và chạy lại verifier.
- Windows CI workflow đã có nhưng chưa chạy trên GitHub, nên Milestone 1 chưa hoàn tất acceptance “cùng command chạy được local và CI”.
- Cargo workspace đã có scanner CLI, SQLite Catalog v1 và M6 one-command fixture demo; chưa có frontend, stable Windows volume/file identity hoặc bất kỳ mutation behavior nào.
- Cargo proxy path cho `cargo clippy` bị treo trên máy hiện tại; verifier Milestone 2 dùng trực tiếp `cargo-clippy` của exact pinned toolchain và cần xác minh lại proxy/tool behavior trước khi chuẩn hóa command cho CI.
- Windows permission/reparse behavior khác CI runner.
- Permission-denied fixture bằng Windows ACL và recovery của orphaned fixture capability trong testkit sau process crash chưa được triển khai; catalog scan-run recovery đã có API và integration test riêng.
- Standard-library path APIs chưa chứng minh chống hostile reparse-swap TOCTOU; M4 chỉ có static reparse no-follow evidence.
- Mtime/access-time có thể thay đổi theo filesystem policy khi đọc; test cần phân biệt hành vi OS với mutation do app.
- Performance target chưa được xác nhận trước benchmark.
- File identity trên non-NTFS cần fallback rõ ràng.
