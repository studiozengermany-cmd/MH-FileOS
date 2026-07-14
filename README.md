# MH FileOS

MH FileOS là ứng dụng desktop local-first dành cho Windows 10/11, giúp người dùng lập chỉ mục, phân tích, tổ chức và phục hồi file một cách có kiểm chứng.

> Trạng thái hiện tại: **EP-000 / Milestone 6 read-only vertical slice đã hoàn tất local checkpoint trên synthetic fixture; UI/Milestone 7 tùy chọn chưa bắt đầu và không có mutation trên dữ liệu người dùng.**

Repository này cố ý bắt đầu bằng đặc tả, safety invariants, kiến trúc và chiến lược kiểm thử. Không module nào được coi là hoàn thành chỉ vì có giao diện hoặc build thành công.

## Tài liệu nguồn sự thật

Đọc theo thứ tự:

1. [`docs/SAFETY-INVARIANTS.md`](docs/SAFETY-INVARIANTS.md) — authority cao nhất và các điều kiện an toàn không được phá vỡ.
2. [`AGENTS.md`](AGENTS.md) — luật bắt buộc cho mọi coding agent.
3. [`PLANS.md`](PLANS.md) — Execution Plan EP-000 đang hoạt động.
4. [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — kiến trúc kỹ thuật và ranh giới module.
5. [`docs/TEST-STRATEGY.md`](docs/TEST-STRATEGY.md) — bằng chứng cần có trước khi phát hành.
6. [`docs/MASTER-PLAN.md`](docs/MASTER-PLAN.md) — phạm vi sản phẩm và lộ trình.

`MH-FILEOS-MASTER-PLAN-MVP-v1.0.md` ở repository root là tài liệu legacy không có authority; mọi xung đột được giải quyết theo danh sách trên.

## Tuyên bố MVP

MVP phải thực hiện được chuỗi sau:

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

MVP không được tự động xóa, ghi đè hoặc di chuyển file ngoài một kế hoạch đã duyệt.

## Cấu trúc hiện tại

```text
MH-FileOS/
├─ AGENTS.md
├─ PLANS.md
├─ README.md
├─ Cargo.toml
├─ Cargo.lock
├─ docs/
│  ├─ MASTER-PLAN.md
│  ├─ SAFETY-INVARIANTS.md
│  ├─ ARCHITECTURE.md
│  ├─ TEST-STRATEGY.md
│  ├─ SAFETY-TRACEABILITY.md
│  └─ adr/
├─ crates/
│  ├─ fileos-domain/
│  ├─ fileos-app/
│  ├─ fileos-testkit/
│  ├─ fileos-catalog/
│  ├─ fileos-scanner/
│  └─ fileos-platform-windows/
├─ apps/
│  ├─ README.md
│  └─ fileos-cli/
├─ scripts/
│  ├─ verify-toolchain.ps1
│  ├─ verify-workspace.ps1
│  └─ verify.ps1
└─ fixtures/
   └─ README.md
```

Workspace được mở rộng từng milestone theo `PLANS.md`; crate chưa đến milestone hành vi chỉ giữ responsibility contract.

## M6 — demo read-only nội bộ

Chạy toàn bộ lát cắt fixture → scan → catalog → summary → xác minh source không đổi bằng một command:

```powershell
cargo run --quiet -p fileos-cli --example fixture-catalog-demo --locked --offline
```

Example chỉ dùng synthetic fixture do testkit tạo, ghi SQLite dưới `artifacts`, xuất JSON không chứa path và chỉ báo `verified` sau khi summary khớp manifest, snapshot trước–sau bằng nhau và sandbox đã cleanup. Command không nhận hoặc quét dữ liệu người dùng.

## Cách giao lượt đầu cho Codex

```text
Đọc tài liệu theo đúng authority order trong README.md.
Chỉ thực hiện milestone EP-000 đang được chủ sản phẩm phê duyệt trong PLANS.md.
Không tạo production feature, không cài dependency và không truy cập ngoài repository.
Trước khi sửa file, hãy tóm tắt phạm vi, rủi ro và tiêu chí hoàn thành.
Sau khi làm xong, cung cấp danh sách file thay đổi và bằng chứng kiểm tra.
```

## Nguyên tắc phát hành

Thứ tự ưu tiên:

1. Không mất dữ liệu.
2. Kết quả đúng.
3. Giải thích rõ.
4. Hiệu năng tốt.
5. Giao diện đẹp.
6. Tự động hóa.

Nếu một tính năng xung đột với an toàn hoặc khả năng phục hồi, tính năng đó phải bị hoãn.
