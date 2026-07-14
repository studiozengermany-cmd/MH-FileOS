# AGENTS.md — MH FileOS Engineering Contract

## 1. Mục đích

File này là hợp đồng vận hành bắt buộc cho mọi coding agent, AI assistant và contributor làm việc trong repository MH FileOS.

MH FileOS can thiệp trực tiếp vào file người dùng. Vì vậy tiêu chuẩn an toàn dữ liệu cao hơn ứng dụng web hoặc công cụ demo. Tốc độ phát triển không được ưu tiên hơn tính đúng, khả năng kiểm chứng và phục hồi.

## 2. Thứ tự nguồn sự thật

Khi tài liệu mâu thuẫn, áp dụng thứ tự:

1. `docs/SAFETY-INVARIANTS.md`
2. `AGENTS.md`
3. Execution Plan đang hoạt động trong `PLANS.md`
4. `docs/ARCHITECTURE.md`
5. `docs/TEST-STRATEGY.md`
6. `docs/MASTER-PLAN.md`
7. Yêu cầu task hiện tại

Không được tự diễn giải một yêu cầu theo cách làm yếu safety invariant. Nếu yêu cầu mới cần thay đổi invariant hoặc kiến trúc đã chốt, dừng và xin phê duyệt bằng một ADR/plan update trước khi code.

## 3. Phạm vi filesystem

### Được phép

- Đọc và ghi trong repository hiện tại.
- Chạy test trên `fixtures/sandbox/` hoặc thư mục tạm do testkit tạo.
- Tạo build artifact trong thư mục build đã định nghĩa.
- Đọc tài liệu công khai cần thiết khi task cho phép truy cập mạng.

### Bị cấm mặc định

- Quét `C:\`, `J:\`, home directory, Downloads thật hoặc bất kỳ thư mục người dùng nào ngoài fixture.
- Di chuyển, đổi tên, xóa hoặc thay nội dung file thật.
- Chạy app hoặc test dưới quyền Administrator nếu không có yêu cầu riêng đã duyệt.
- Ghi ra ngoài repository hoặc test temp directory.
- Theo junction/symlink/reparse point ra ngoài fixture.
- Dùng đường dẫn hard-code theo máy của contributor.

Mọi test filesystem phải có một `scope root` rõ ràng và kiểm tra canonical path vẫn nằm bên trong scope trước khi ghi.

## 4. Các lệnh cần xin phê duyệt

Không tự chạy các thao tác sau nếu task chưa cho phép rõ:

- Cài hoặc cập nhật dependency production.
- `npm install`, `cargo add`, `winget`, `choco`, `pip install` hoặc tương đương.
- Xóa file/thư mục, kể cả build cache không do task hiện tại tạo.
- Force push, rewrite history, reset, clean hoặc checkout phá thay đổi.
- Bật quyền admin/full disk access.
- Tải binary hoặc chạy script từ Internet.
- Publish package, release, deployment hoặc upload dữ liệu.
- Thay đổi CI secret, signing key hoặc policy bảo mật.

Nếu cần dependency mới, phải báo:

1. Tên và version dự kiến.
2. Vì sao standard library/dependency hiện có không đủ.
3. License.
4. Ảnh hưởng kích thước, security và maintenance.
5. Phương án không thêm dependency.

## 5. Quy trình bắt buộc cho mỗi task

### Bước A — Orient

Trước khi sửa:

- Đọc file hướng dẫn áp dụng cho thư mục.
- Kiểm tra git status và giữ nguyên thay đổi không liên quan.
- Xác định task thuộc milestone nào.
- Liệt kê file dự kiến đọc/sửa.
- Nêu rõ rủi ro dữ liệu, migration, hiệu năng hoặc compatibility.

### Bước B — Plan

Task nhỏ cần một kế hoạch ngắn trong phản hồi. Task phức tạp, kéo dài hoặc chạm nhiều module phải cập nhật Execution Plan theo `PLANS.md`.

Kế hoạch phải có:

- Outcome.
- Scope và non-scope.
- Acceptance criteria.
- Cách kiểm thử.
- Rollback/recovery nếu có mutation.

### Bước C — Implement

- Thực hiện lát cắt nhỏ nhất đáp ứng acceptance criteria.
- Không tự mở rộng phạm vi vì “tiện thể”.
- Không refactor file không liên quan.
- Không viết UI giả cho backend chưa tồn tại.
- Không thêm provider/API/setting không được sử dụng thật.
- Không để `TODO`, placeholder hoặc nhánh fake success trong production path.

### Bước D — Verify

Chạy kiểm tra tỷ lệ với rủi ro:

- Format/lint.
- Unit tests bị ảnh hưởng.
- Integration tests nếu chạm filesystem/database.
- Fault injection nếu chạm executor/journal/recovery.
- Review diff.
- Ghi rõ kiểm tra nào đã chạy, kết quả nào pass/fail và phần nào chưa test.

### Bước E — Handoff

Kết thúc bằng:

- Outcome thực tế.
- File thay đổi.
- Bằng chứng kiểm thử.
- Rủi ro còn lại.
- Milestone tiếp theo, nhưng không tự bắt đầu milestone đó.

Không chạy quá ba nhóm lệnh liên tiếp mà không tạo một checkpoint cập nhật ngắn khi task đang được người dùng theo dõi trực tiếp.

## 6. Luật kiến trúc

### 6.1. Core độc lập UI

- Rust core không phụ thuộc React.
- Domain và state machine không phụ thuộc Tauri.
- Tauri command chỉ chuyển DTO, gọi use case và map error typed.
- React không được trực tiếp thao tác filesystem.

### 6.2. Query không mutation

Command được đặt tên/thiết kế là query không được thay đổi file người dùng. Scan và analysis phải read-only.

### 6.3. Mọi mutation đi qua plan

Không module nào được gọi `rename`, `copy`, `remove_file` hoặc API tương đương cho dữ liệu người dùng ngoài Transactional Executor.

Luồng bắt buộc:

```text
Finding/Rule → Immutable Plan → Validation → Approval → Executor → Verification → Journal
```

### 6.4. Không nối chuỗi shell command với path

- Không dùng `cmd /c`, PowerShell command string hoặc shell interpolation để mở/move file.
- Dùng OS API hoặc process arguments typed.
- Mọi path lạ phải có test: dấu nháy, `&`, Unicode, emoji, khoảng trắng, path dài.

### 6.5. Error typed

- Không điều khiển UI bằng cách parse message string.
- Error phải có code, user-safe message, recoverability và context đã giảm nhạy cảm.
- Không log nội dung file.
- Đường dẫn trong diagnostic export phải được redaction theo policy.

### 6.6. Database

- Migration có version và test upgrade.
- Không `unwrap()` dữ liệu lấy từ DB ở production path.
- Không giữ DB lock trong khi thực hiện I/O filesystem dài.
- Ghi batch và transaction rõ ràng.
- Operation journal là nguồn sự thật cho mutation; bảng history UI không thay thế journal.

### 6.7. Concurrency

- Mọi queue I/O phải bounded.
- Có cancellation token.
- Không spawn task vô hạn theo số file.
- Concurrency phải nhận biết loại volume; tránh seek storm trên HDD/network.
- UI event phải aggregate/throttle.

## 7. Luật an toàn file operation

Các invariant đầy đủ nằm trong `docs/SAFETY-INVARIANTS.md`. Tóm tắt không thể thương lượng:

1. Không xóa source trước khi destination được xác minh cho cross-volume move.
2. Không overwrite mặc định.
3. Không báo success trước trạng thái `committed`.
4. Không đánh dấu undo thành công trước `undo_committed`.
5. Không thực thi plan stale.
6. Không write protected zone theo policy mặc định.
7. Không cho loại bỏ toàn bộ bản trong duplicate group.
8. Crash ở mỗi checkpoint phải dẫn tới trạng thái phục hồi xác định.
9. AI output không được biến trực tiếp thành destructive operation.
10. Không thay đổi file trong scan/analysis.

Nếu code đề xuất phá một invariant, không được merge dù test khác đang pass.

## 8. Chuẩn code

### Rust

- `cargo fmt` bắt buộc.
- `cargo clippy` không có warning trong code mới.
- Public API có doc comment khi hành vi không hiển nhiên.
- Ưu tiên type/state machine thay cho boolean flags mơ hồ.
- Không dùng `unsafe` nếu chưa có ADR, safety comment và test riêng.
- Không `panic!`, `unwrap()` hoặc `expect()` trên input/runtime path có thể xảy ra.
- Path dùng `Path`/`PathBuf`, không biến thành string sớm.
- Kết quả I/O không bị bỏ qua bằng `let _ =` trong mutation path.

### TypeScript/React

- TypeScript strict.
- Không `any` trong production code nếu không có lý do/documentation.
- Component không chứa business logic filesystem.
- Loading/empty/error/success là trạng thái thật.
- Danh sách lớn phải pagination hoặc virtualization.
- Text hiển thị đi qua i18n; tiếng Việt và Anh là bắt buộc cho P0.

### SQL

- Query parameterized.
- Schema change qua migration.
- Index có lý do và benchmark khi ảnh hưởng catalog lớn.
- Không SELECT toàn bộ hàng triệu row đưa lên UI.

## 9. Chính sách dependency

- Lock version theo cơ chế của ecosystem.
- Chỉ dùng crate/package còn maintain, license tương thích và có nhu cầu thật.
- Security-critical functionality ưu tiên thư viện trưởng thành hoặc OS API.
- Không thêm dependency chỉ để tránh viết vài dòng code rõ ràng.
- Không bật telemetry SDK trong MVP.
- Không thêm network client vào core nếu feature hiện tại không dùng mạng.

## 10. Chính sách test

Mức test tối thiểu:

| Thay đổi | Bằng chứng bắt buộc |
|---|---|
| Domain/rule/path | Unit + property test phù hợp |
| Scanner | Unit + fixture integration + cancellation |
| Catalog/migration | Migration + integration DB |
| Duplicate | Golden fixture + stale fingerprint |
| Planner | Conflict/protected/stale plan tests |
| Executor | Integration + fault injection |
| Undo/recovery | Crash checkpoint matrix |
| UI P0 | Component + critical flow E2E |
| Performance | Benchmark trước/sau nếu có nguy cơ regression |

Không được dùng file người dùng làm fixture. Test phải tự tạo dữ liệu và tự cleanup trong root riêng.

## 11. Bằng chứng và nhãn báo cáo

Sử dụng đúng nhãn:

- **[ĐÃ XÁC MINH BẰNG TEST]** — có command và kết quả.
- **[ĐÃ XÁC MINH BẰNG FILE]** — đối chiếu mã/tài liệu, chưa chạy.
- **[CHƯA KIỂM THỬ]** — chưa có runtime evidence.
- **[SUY LUẬN]** — nhận định kỹ thuật cần xác minh.

Không dùng “hoàn hảo”, “production-ready”, “an toàn tuyệt đối” hoặc “không có lỗi” nếu thiếu test matrix và release gate.

## 12. Git và checkpoint

- Một branch cho một milestone/lát cắt.
- Commit nhỏ, message mô tả outcome.
- Không commit binary, database runtime, user data hoặc secret.
- Không force push nếu chưa được yêu cầu.
- Trước commit: review `git diff`, test liên quan, kiểm tra file ngoài scope.
- Không để hai agent cùng sửa một worktree. Tác vụ song song phải dùng worktree riêng và file ownership không chồng lấn.

## 13. Điều kiện phải dừng

Dừng và xin quyết định khi:

- Cần thay safety invariant.
- Cần ghi ngoài repository/fixture.
- Cần quyền admin.
- Cần dependency production mới chưa duyệt.
- Phát hiện thay đổi người dùng chồng lên file cần sửa.
- Acceptance criteria mâu thuẫn.
- Test cho thấy khả năng mất dữ liệu hoặc false-success.
- Không thể xác minh hành vi trên Windows 10/11.
- Scope có nguy cơ vượt milestone hiện tại.

## 14. Definition of Done

Một task chỉ Done khi:

1. Acceptance criteria đạt.
2. Không phá safety invariant.
3. Không placeholder/TODO trong production path.
4. Format/lint pass.
5. Test phù hợp pass.
6. Error và recovery path đã xử lý.
7. Tài liệu/contract cập nhật nếu hành vi thay đổi.
8. Diff được review.
9. Bằng chứng được báo trung thực.
10. Không có file ngoài scope bị thay đổi.

Build thành công nhưng chưa chạy test không phải Done.

## 15. Trạng thái repository hiện tại

- Phase: Read-only Foundation / Sprint 0.
- Read-only foundation: Milestone 6 read-only vertical slice đã hoàn tất local checkpoint trong synthetic fixture sandbox; UI/Milestone 7 tùy chọn chưa bắt đầu.
- Scope mặc định: tài liệu, workspace contracts, synthetic fixture sandbox và catalog database dưới test artifacts; không dùng dữ liệu người dùng làm test.
- Execution Plan hoạt động: `EP-000` trong `PLANS.md`.
- Tính năng tự động move/delete: bị cấm cho đến khi các gate trong `docs/TEST-STRATEGY.md` đạt.
