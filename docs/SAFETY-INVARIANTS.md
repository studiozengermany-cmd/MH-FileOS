# MH FileOS — Safety Invariants

**Authority:** Tài liệu có mức ưu tiên cao nhất trong repository.  
**Applies to:** Scanner, Catalog, Analysis, Planner, Executor, Journal, Undo, Watcher, UI và testkit.  

## 1. Mục đích

Safety invariant là điều luôn phải đúng ở mọi trạng thái được hỗ trợ, kể cả khi:

- App crash.
- Máy mất điện.
- File bị chương trình khác thay đổi.
- Volume bị tháo.
- Destination hết dung lượng.
- Permission thay đổi.
- Người dùng đóng app giữa operation.

Nếu feature không chứng minh được invariants liên quan, feature chưa đủ điều kiện merge/release.

## 2. Thuật ngữ

- **Source:** File/path trước operation.
- **Destination:** File/path mục tiêu.
- **Snapshot:** Metadata/identity/fingerprint dùng để lập plan.
- **Plan:** Danh sách operation bất biến đã tạo từ snapshot.
- **Committed:** Filesystem state đã verify và journal ghi hoàn tất.
- **Retire source:** Loại source khỏi vị trí cũ bằng move-to-quarantine hoặc delete theo policy.
- **Quarantine:** Vùng phục hồi do app quản lý.
- **Protected Zone:** Scope không được write theo policy mặc định.
- **Stale:** Filesystem state không còn khớp snapshot.

## 3. Invariants P0

### SI-001 — Read-only means read-only

Scan, catalog query, analysis, simulation và preview không được:

- Tạo, sửa, đổi tên, move hoặc xóa user file.
- Thay permissions/attributes.
- Giải nén archive.
- Ghi marker vào scan root.

Việc OS tự cập nhật access time do read phải được ghi nhận như platform behavior và không được app chủ động gây thêm mutation.

### SI-002 — Scope confinement

Mọi write phải có approved scope. Canonical destination và mọi parent phải nằm trong scope được phép hoặc destination scope được người dùng duyệt riêng.

Junction/symlink/reparse point không được dùng để thoát scope.

### SI-003 — No source retirement before destination verification

Đối với cross-volume move:

```text
copy temp → flush/close → verify → promote destination → retire source
```

Không được retire source trước `verified`.

### SI-004 — No silent overwrite

Nếu destination tồn tại:

- Operation chuyển `blocked_conflict`.
- Mặc định không ghi đè.
- Resolution phải là dữ liệu rõ ràng trong plan.
- “Replace” cần confirmation nâng cao và backup/quarantine policy.

### SI-005 — Truthful success

UI/API chỉ trả success khi operation ở `committed`. Không bỏ qua kết quả I/O. Không dùng `let _ =` cho mutation result.

### SI-006 — Plan freshness

Trước execute, source phải khớp snapshot theo policy:

- Expected existence.
- Size.
- Modified time.
- Platform file identity khi có.
- Fingerprint khi operation cần content certainty.

Mismatch làm item `blocked_stale`, không tự update plan và tiếp tục.

### SI-007 — Deterministic recovery

Mỗi durable checkpoint phải cho phép một trong:

- Resume an toàn.
- Rollback an toàn.
- Inspect/attention required với cả dữ liệu còn nguyên.

Không để app báo “không biết file ở đâu” nếu journal có thể ghi đủ thông tin từ trước.

### SI-008 — Undo must be verified

Undo chỉ `undo_committed` sau khi state phục hồi được verify. Nếu source path cũ bị chiếm hoặc file đã đổi, báo conflict.

### SI-009 — Duplicate group retains at least one copy

UI và core validation không cho một plan loại bỏ/quarantine toàn bộ member của confirmed duplicate group, trừ workflow khác có explicit external backup evidence; workflow đó không thuộc MVP.

### SI-010 — No shell interpolation of paths

Không đưa path/file name vào command string của `cmd`, PowerShell, Bash hoặc shell khác. Dùng OS API hoặc process arguments typed.

### SI-011 — Protected Zones are enforced in core

UI disable không đủ. Planner và Executor đều phải kiểm tra protection. Protected policy không thể bypass bằng gọi command trực tiếp.

### SI-012 — AI is advisory only

AI/ML output có thể tạo label/suggestion nhưng không trực tiếp tạo approved destructive operation. Plan cần deterministic validation và user approval.

### SI-013 — Test isolation

Test mutation chỉ chạy trong sandbox được testkit tạo. Cleanup phải từ chối root không mang marker/identity của testkit và canonical path ngoài temp base.

### SI-014 — Journal precedes mutation

Durable intent phải được ghi trước filesystem mutation đầu tiên. Journal event append-only; correction dùng event mới, không sửa lịch sử đã ghi.

### SI-015 — UI history is not operation truth

History có thể là projection/cache. Recovery luôn dựa trên journal + filesystem validation.

### SI-016 — Cancellation occurs at safe points

Cancel không kill thread giữa write tùy ý. Executor hoàn tất/rollback atomic step hiện tại rồi chuyển trạng thái xác định.

### SI-017 — Evidence labels are truthful

Không gọi runtime behavior “đã test” nếu mới đọc code hoặc build. Báo rõ File/Test/Untested/Inference.

### SI-018 — No unbounded resource growth

- Queue bounded.
- Concurrency bounded.
- UI event throttled.
- Hash scheduling staged.
- Archive import sau MVP phải giới hạn entries/size/ratio.

### SI-019 — User data never becomes telemetry by default

Không gửi path, filename, hash, metadata hoặc nội dung. Diagnostic export phải opt-in và redacted.

### SI-020 — Versioned binary identity

Binary thay đổi phải có version/build identity mới. Release có checksum. Không silent refresh cùng version.

## 4. Operation state machines

## 4.1. Same-volume move

```text
planned
→ validated
→ intent_recorded
→ rename_started
→ destination_verified
→ committed
```

Failure states:

- `blocked_stale`
- `blocked_conflict`
- `failed_recoverable`
- `failed_attention_required`
- `rolled_back`

Atomic rename failure không được tự động fallback sang copy/delete nếu plan/policy không cho phép. Nếu fallback được hỗ trợ, nó là transition rõ sang cross-volume-like strategy.

## 4.2. Cross-volume move

```text
planned
→ validated
→ intent_recorded
→ temp_destination_created
→ copying
→ copied
→ destination_verified
→ destination_promoted
→ source_retirement_started
→ source_retired
→ committed
```

Journal cần đủ dữ liệu:

- Source/destination/temp path.
- Snapshot/fingerprint.
- Byte progress khi cần resume.
- Strategy.
- Verification result.
- Source retirement policy.

## 4.3. Quarantine

Quarantine là move có journal. Metadata phải lưu:

- Original path.
- Quarantine path.
- Reason/plan.
- Fingerprint.
- Created/expiry policy.
- Restore state.

MVP không tự purge Quarantine.

## 4.4. Undo

```text
undo_planned
→ undo_validated
→ undo_intent_recorded
→ reverse_operation
→ restored_verified
→ undo_committed
```

Nếu original path tồn tại, không overwrite; yêu cầu resolution.

## 5. Plan validation checklist

Mỗi item phải kiểm tra:

- Source tồn tại hoặc trạng thái expected phù hợp.
- Snapshot fresh.
- Source nằm trong approved read/write scope.
- Destination canonical hợp lệ.
- Destination parent không đi qua protected/reparse escape.
- Destination không trùng item khác trong plan.
- Không move directory vào descendant của chính nó.
- Free space đủ với safety margin.
- Lock/permission policy.
- Volume hiện diện.
- Conflict resolution hợp lệ.
- Undo feasibility được tính và hiển thị.

Plan-level validation:

- Không duplicate group mất hết bản.
- Không cycle giữa move operations.
- Thứ tự operation không làm source của item khác biến mất.
- Estimated bytes per volume hợp lý.

## 6. Protected Zones

### Blocked mặc định

- Windows directory.
- Program Files/Program Files (x86).
- ProgramData system-managed areas.
- System Volume Information.
- Recycle Bin internals.
- App install/data/journal đang hoạt động.

### Read-only/confirm mặc định

- AppData.
- Cloud sync roots.
- Git repositories.
- IDE/build environments.
- DAW/video project roots.
- Removable/network volume.
- Database-containing directories.
- Roots có reparse points phức tạp.

### Project Zone heuristics MVP

Nếu root/subtree có các marker sau, nâng protection:

- `.git`
- `package.json` + lockfile
- Cargo workspace
- `.sln`, `.csproj`
- `.flp`
- project files của editor được hỗ trợ trong heuristic registry

Heuristic chỉ tăng bảo vệ; không được giảm bảo vệ.

## 7. Duplicate safety

Trạng thái:

- `candidate_same_size`
- `candidate_quick_match`
- `confirmed_full_hash`
- `confirmed_byte_equal`
- `stale_requires_recheck`

Chỉ `confirmed_full_hash` trở lên mới cho tạo Quarantine plan. Trước execute phải revalidate.

Hardlink cần được phân biệt với hai physical copies để không báo dung lượng thu hồi sai.

## 8. Watcher safety

- Chỉ watch explicit Inbox roots.
- Không recursive toàn ổ mặc định.
- Chờ stability window.
- Bỏ file temp download.
- Lock check.
- Event coalescing.
- Watcher chỉ tạo suggestion trong MVP.
- Trusted/Auto Mode thuộc policy riêng sau MVP và vẫn đi qua executor/journal.

## 9. Diagnostic and privacy

Diagnostic bundle mặc định chỉ có:

- App/version/OS.
- Error codes.
- Operation states.
- Performance counters aggregate.
- Paths đã redacted hoặc tokenized.

Không bao gồm:

- File content.
- Full path thô.
- Filename thô nếu chưa opt-in.
- Usernames/home path.
- Hash dùng để nhận diện dữ liệu nếu không cần.

## 10. Review requirements

Thay đổi liên quan SI-003 đến SI-009 hoặc SI-014 bắt buộc:

- Execution Plan.
- State transition review.
- Integration tests.
- Fault injection.
- Diff review độc lập.
- Ghi remaining risks.

Không được giảm invariant để làm test pass. Phải sửa implementation hoặc giảm scope feature.

