# MH FileOS

> **Trạng thái:** dự án cá nhân đang nghiên cứu và phát triển. Hiện mới hoàn tất lát cắt read-only nội bộ trên dữ liệu thử nghiệm; chưa phải phần mềm thương mại và chưa sẵn sàng dùng trên dữ liệu thật của người dùng.

MH FileOS được Minh Hiếu xây dựng trước hết để phục vụ nhu cầu cá nhân: hiểu rõ dữ liệu trên máy Windows, lập chỉ mục file, phát hiện vấn đề và chuẩn bị thao tác theo cách có thể kiểm tra, giải thích và phục hồi.

Khi dự án đủ ổn định và an toàn, một số phần có thể được chia sẻ để cộng đồng tham khảo hoặc dùng thử. README này chỉ mô tả những gì đã có bằng chứng; không coi giao diện, build hoặc mã khung là tính năng đã hoàn thành.

## Nguyên tắc cốt lõi

Thứ tự ưu tiên của dự án:

1. Không làm mất dữ liệu.
2. Kết quả phải đúng.
3. Mọi hành động phải giải thích được.
4. Có kiểm chứng và khả năng phục hồi.
5. Hiệu năng tốt.
6. Giao diện đẹp.
7. Tự động hóa chỉ được thêm sau cùng.

Nếu một tính năng xung đột với an toàn hoặc khả năng phục hồi, tính năng đó phải bị hoãn.

## Trạng thái hiện tại

`EP-000 / Milestone 6` đã hoàn tất một lát cắt read-only trên synthetic fixture do testkit tạo.

Điều này có nghĩa:

- Có quy trình tạo dữ liệu thử nghiệm, scan, catalog, tổng hợp kết quả và xác minh source không đổi.
- Có SQLite artifact và JSON summary dùng cho kiểm chứng nội bộ.
- Command demo không nhận hoặc quét dữ liệu thật của người dùng.
- UI chưa bắt đầu.
- Chưa có thao tác thay đổi dữ liệu người dùng.
- Chưa có bản phát hành ổn định hoặc installer chính thức cho người dùng cuối.

## Mục tiêu MVP

MVP chỉ được coi là đạt khi thực hiện được đầy đủ chuỗi sau:

```text
Scan read-only
→ Catalog
→ Findings
→ Action Plan
→ User Approval
→ Transactional Execution
→ Verification
→ Journal
→ Undo/Recovery
```

MVP không được tự động xóa, ghi đè hoặc di chuyển file ngoài một kế hoạch đã được người dùng xem và chấp thuận.

## Demo read-only nội bộ

```powershell
cargo run --quiet -p fileos-cli --example fixture-catalog-demo --locked --offline
```

Demo hiện tại chỉ chạy với dữ liệu giả lập. Kết quả chỉ được đánh dấu `verified` khi summary khớp manifest, snapshot trước–sau bằng nhau và sandbox đã được dọn sạch.

## Tài liệu nguồn sự thật

Đọc theo thứ tự:

1. [`docs/SAFETY-INVARIANTS.md`](docs/SAFETY-INVARIANTS.md)
2. [`AGENTS.md`](AGENTS.md)
3. [`PLANS.md`](PLANS.md)
4. [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
5. [`docs/TEST-STRATEGY.md`](docs/TEST-STRATEGY.md)
6. [`docs/MASTER-PLAN.md`](docs/MASTER-PLAN.md)

`MH-FILEOS-MASTER-PLAN-MVP-v1.0.md` ở root là tài liệu legacy và không có authority khi xung đột với danh sách trên.

## Cấu trúc hiện tại

```text
MH-FileOS/
├─ AGENTS.md
├─ PLANS.md
├─ README.md
├─ Cargo.toml
├─ Cargo.lock
├─ docs/
├─ crates/
├─ apps/
├─ scripts/
└─ fixtures/
```

Workspace được mở rộng theo từng milestone. Một crate tồn tại trong source không đồng nghĩa toàn bộ hành vi của crate đó đã hoàn thành.

## Quy tắc làm việc với coding agent

```text
Đọc tài liệu theo đúng authority order trong README.md.
Chỉ thực hiện milestone đang được chủ dự án phê duyệt trong PLANS.md.
Không tự tạo production feature, không tự cài dependency và không truy cập ngoài repository.
Trước khi sửa file, tóm tắt phạm vi, rủi ro và tiêu chí hoàn thành.
Sau khi hoàn tất, cung cấp danh sách file thay đổi và bằng chứng kiểm tra.
Không xóa, khóa hoặc làm mất lịch sử dự án khi chưa có chỉ dẫn rõ ràng từ chủ dự án.
```

## Quyền sử dụng và chia sẻ

Dự án hiện được công khai để minh bạch quá trình nghiên cứu và phát triển. Không nên sử dụng trên dữ liệu quan trọng cho đến khi có hướng dẫn phát hành, kiểm thử và giấy phép rõ ràng.

## Liên hệ

- Website: https://studiominhhieu.com/
- Email: support@studiominhhieu.com
- GitHub: https://github.com/studiozengermany-cmd
