# MH FileOS — Master Product Plan

**Version:** 1.0  
**Platform MVP:** Windows 10/11  
**Product mode:** Local-first, offline core  
**Status:** Approved direction; implementation begins only through `PLANS.md`  

**Implementation checkpoint (2026-07-27):** EP-000 M0–M6 local checkpoints pass; EP-000 remains `VERIFYING` pending Windows CI and safety acceptance. Production CLI supports read-only scan of one absolute directory; automated evidence remains fixture-only. Phase 2, UI, installer and user-file mutation have not started.

## 1. Product statement

MH FileOS là trung tâm quản lý file cá nhân có khả năng:

- Lập chỉ mục kho file trong phạm vi người dùng cấp quyền.
- Giải thích dung lượng và cấu trúc dữ liệu.
- Phát hiện file trùng, file lớn, file rỗng và khu vực lộn xộn.
- Đề xuất cách tổ chức bằng rule có thể kiểm tra.
- Tạo kế hoạch bất biến trước khi thay đổi file.
- Thực hiện thay đổi có journal, verification và recovery.
- Giữ máy gọn về sau bằng Inbox Suggest Mode.

Sản phẩm không được định vị như “cleaner tăng tốc máy”. Nó là lớp an toàn và tổ chức dữ liệu nằm trên filesystem.

## 2. Product promise

> Quét kho file, chỉ ra thứ đang chiếm chỗ hoặc lộn xộn, giúp người dùng tổ chức và xử lý bản trùng bằng kế hoạch có thể kiểm tra, xác minh và hoàn tác.

Ba lời hứa bắt buộc:

1. **Không hành động bí mật:** Người dùng nhìn thấy trước thay đổi.
2. **Không báo thành công giả:** Kết quả chỉ thành công sau verification.
3. **Không chiếm quyền kiểm soát:** Auto Mode không bật mặc định.

## 3. Target users

### P0 — Người dùng có kho file nhiều năm

- Không biết dung lượng nằm ở đâu.
- Có nhiều file trùng nhưng sợ xóa.
- Cần kết quả dễ hiểu, không cần biết hash hay filesystem.

### P0 — Người sáng tạo nội dung

- Có ảnh, video, âm thanh, project và asset liên kết.
- Di chuyển sai path có thể phá project.
- Cần vùng bảo vệ và kế hoạch rõ ràng.

### P1 — Người có Downloads/Inbox luôn lộn xộn

- Muốn file mới được đề xuất vị trí tự động.
- Chỉ bật automation sau khi đã tin rule.

### P1 — Power user

- Cần regex, rule set, export plan và log chi tiết.

## 4. Product principles

1. Local-first.
2. Read broadly, write narrowly.
3. Plan before mutation.
4. Quarantine before permanent deletion.
5. Never overwrite by default.
6. Explain every proposal.
7. Deterministic core; AI optional after MVP.
8. Project/reference safety before cleanup gains.
9. Build evidence is not test evidence.
10. A feature without recovery is incomplete.

## 5. MVP capabilities

### FR-001 — Scope selection

Người dùng chọn một hoặc nhiều root. App phải cho biết root nào read-only, protected hoặc writable theo policy.

### FR-002 — Quick Scan

Thu metadata, path, size, timestamps, attributes và platform identity nếu có. Không full hash toàn bộ.

### FR-003 — Deep Scan

Bao gồm Quick Scan, fingerprint theo nhu cầu và metadata adapter được hỗ trợ.

### FR-004 — Catalog

Lưu snapshot có thể query, update và phát hiện stale state.

### FR-005 — Storage dashboard

Hiển thị tổng dung lượng, category, top directory, top file và lỗi. Mọi số liệu phải drill-down được.

### FR-006 — Exact duplicates

Phát hiện bằng size → quick fingerprint → full hash → optional byte compare.

### FR-007 — Large/empty findings

Cho biết file lớn, file 0 byte và thư mục rỗng. Không gọi chúng là rác mặc định.

### FR-008 — Scoped Rule Sets

Rule thuộc root/inbox cụ thể, có priority và simulator.

### FR-009 — Action Plan

Mọi move/rename/quarantine phải thành plan có source snapshot, destination, reason, risk và conflict state.

### FR-010 — Plan validation

Revalidate source, destination, free space, protection, collision và freshness trước Execute.

### FR-011 — Transactional execution

Same-volume dùng atomic rename khi phù hợp. Cross-volume dùng copy → verify → retire source → commit.

### FR-012 — Journal

Mỗi operation có event/state phục vụ recovery; UI history không phải nguồn sự thật duy nhất.

### FR-013 — Quarantine

Đưa bản thừa hoặc item cần loại bỏ vào vùng phục hồi được.

### FR-014 — Undo

Reverse transaction có verification, conflict handling và support sau restart.

### FR-015 — Protected Zones

Block/read-only/confirm policy cho hệ thống, cloud sync, project, repository, removable/network roots.

### FR-016 — Inbox Suggest Mode

Watcher tạo đề xuất cho file ổn định; không tự move mặc định.

### FR-017 — History & Recovery

Hiển thị plan/operation state trung thực; startup phát hiện incomplete journal.

### FR-018 — Vietnamese/English UI

Toàn bộ P0 flow và error phải có tiếng Việt và Anh.

## 6. Explicit non-goals for MVP

- Registry cleaner.
- RAM booster.
- Similar photo AI.
- Semantic content search.
- Cloud account/sync service riêng.
- Parse đầy đủ FL Studio/Adobe project dependency.
- macOS/Linux public release.
- Automatic permanent deletion.
- Full-disk Auto Mode.
- Installer chạy admin mặc định.

## 7. User journey

### Journey A — First useful result

1. App mở ở read-only mode.
2. Người dùng chọn một thư mục.
3. App hiển thị protection/scope.
4. Quick Scan chạy có progress.
5. Dashboard hiển thị category, large file, duplicate candidates.
6. Người dùng drill down vào bằng chứng.

Success: Người dùng nhận giá trị mà chưa có mutation.

### Journey B — Duplicate cleanup

1. App nhóm confirmed duplicates.
2. Người dùng xem bản đề xuất giữ và lý do.
3. Người dùng chọn bản thừa.
4. App tạo plan đưa vào Quarantine.
5. Validate lại.
6. Execute và verify.
7. History hiển thị committed.
8. Restore/undo khả dụng.

### Journey C — Organize folder

1. Người dùng chọn/scoped rule set.
2. Simulator cho biết rule nào match.
3. App tạo Before/After plan.
4. Conflict được resolve.
5. Execute có journal.
6. Người dùng xem report.

### Journey D — Inbox suggestion

1. Watcher phát hiện file mới.
2. Chờ file ổn định và unlock.
3. Rule tạo suggestion.
4. Người dùng duyệt batch.
5. Rule có thể được đánh dấu trusted sau nhiều lần thành công; Auto Mode thuộc phase sau.

## 8. Screens

| Screen | P0 outcome |
|---|---|
| Onboarding | Chọn scope và hiểu read-only |
| Overview | Thấy tình trạng và việc cần chú ý |
| Scan | Tạo/quản lý scan run |
| Results | Drill-down mọi finding |
| Duplicates | Chọn bản giữ/Quarantine an toàn |
| Plan Review | Hiểu Before/After/risk/conflict |
| Execution | Theo dõi state thật |
| Rules | Tạo, scope và simulate rule |
| History & Recovery | Undo/recover/inspect |
| Protected Zones | Xem và quản lý policy |
| Settings | Language, resource, privacy |

## 9. Product success metrics

### Integrity metrics

- Zero confirmed data-loss incident trong private beta.
- Zero silent overwrite.
- 100% supported committed operation có kết quả undo xác định trong test matrix.
- 100% destructive action có audit trail.

### Experience metrics

- First useful finding mục tiêu dưới 5 phút với root thông thường.
- Trên 70% beta user hoàn tất first scan.
- Trên 50% beta user tạo ít nhất một plan.
- Recovery issue phải nổi cao hơn recommendation mới.

### Performance targets

Các số sau là target phải benchmark trước khi quảng cáo:

- App interactive dưới 2 giây trên máy tham chiếu.
- Filter/sort indexed dưới 100 ms.
- Quick Scan 100k file SSD mục tiêu dưới 3 phút.
- Catalog 1M entry vẫn query/paginate được.
- Idle CPU trung bình dưới 1% khi không scan/watch.
- Idle memory mục tiêu dưới 200 MB.

## 10. Phase roadmap

### Phase 0 — Repository Contract

- Docs, invariants, architecture, test strategy, execution plan.

### Phase 1 — Read-only Foundation

- Fixture, scanner, catalog, CLI summary.

### Phase 2 — Analysis

- Category, large/empty, exact duplicate.

### Phase 3 — Planning

- Rules, protected zones, immutable plan, conflicts.

### Phase 4 — Mutation Core

- Transaction executor, journal, quarantine.

### Phase 5 — Recovery

- Undo, crash recovery, history UI.

### Phase 6 — Desktop Product

- Full UI, i18n, accessibility, performance.

### Phase 7 — Suggest Mode

- Watcher và trusted rule flow.

### Phase 8 — Hardening/Beta

- Security, fault injection, clean VM, signing, beta soak.

## 11. Release gates

Không public release nếu:

- Cross-volume chưa verify.
- Undo có false-success.
- Crash checkpoint không recover được.
- Protected zone bypass được qua junction.
- Duplicate flow cho loại bỏ toàn bộ bản.
- Binary cùng version nhưng byte thay đổi im lặng.
- CI chỉ build, không test.
- Chưa test clean Windows 10/11 VM.

Public MVP cần:

- Toàn bộ P0 acceptance pass.
- Fault-injection suite pass.
- Security review pass.
- Private beta tối thiểu hai tuần không có P0/P1 integrity bug.
- Signed/versioned artifact và checksum.
- Recovery guide.

## 12. Product decisions pending

Các quyết định không chặn EP-000 nhưng phải chốt trước UI/release:

1. Tên thương hiệu cuối cùng.
2. Free-only hay nền Free/Pro.
3. Quarantine per-volume hay central.
4. Cho chọn whole drive ngay onboarding hay ẩn dưới Advanced.
5. Suggest Mode nằm cuối MVP hay 1.1.

Không agent nào được tự quyết các mục này bằng code.

