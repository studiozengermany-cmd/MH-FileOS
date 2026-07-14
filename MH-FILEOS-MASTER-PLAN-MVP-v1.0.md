# MH FILEOS — MASTER PRODUCT & ENGINEERING PLAN

> **LEGACY — NON-AUTHORITATIVE.** Tài liệu lịch sử này được giữ để tham khảo nguồn gốc sản phẩm nhưng không điều khiển implementation hoặc test. Nguồn sự thật hiện hành là `docs/SAFETY-INVARIANTS.md` → `AGENTS.md` → `PLANS.md` → `docs/ARCHITECTURE.md` → `docs/TEST-STRATEGY.md` → `docs/MASTER-PLAN.md`. Mọi đề xuất dùng bản sao dữ liệu người dùng thật trong tài liệu này đã bị supersede; automated test chỉ dùng synthetic fixture trong sandbox do `fileos-testkit` tạo.

**Phiên bản tài liệu:** 1.0  
**Ngày lập:** 13/07/2026  
**Tên mã sản phẩm:** MH FileOS *(tên làm việc, chưa phải thương hiệu cuối cùng)*  
**Nền tảng MVP:** Windows 10/11, desktop, local-first  
**Trạng thái:** Legacy reference; superseded by repository contracts

---

## 0. TUYÊN BỐ SẢN PHẨM

MH FileOS không phải là một ứng dụng “dọn Downloads” mở rộng. Sản phẩm được định nghĩa là:

> **Trung tâm quản lý file cá nhân chạy hoàn toàn trên máy, có khả năng lập chỉ mục, phân tích, phát hiện vấn đề, đề xuất cách tổ chức, thực thi thay đổi có kiểm chứng và phục hồi an toàn.**

Mục tiêu cuối cùng là khiến người dùng cảm thấy đây là một ứng dụng không thể thiếu vì nó trả lời được năm câu hỏi mà hệ điều hành hiện tại trả lời rất kém:

1. Máy của tôi đang chứa những gì?
2. Dung lượng đang bị chiếm bởi đâu và vì sao?
3. File nào trùng, rác, sai vị trí hoặc có nguy cơ thất lạc?
4. Nếu sắp xếp lại, chính xác file nào sẽ đi đâu và có ảnh hưởng gì?
5. Nếu có sự cố, tôi có thể phục hồi chắc chắn hay không?

MH FileOS phải tạo được cảm giác “an tâm khi giao dữ liệu”, không chỉ cảm giác “nhiều tính năng”.

---

## 1. PHẠM VI VÀ CÁC NHÃN TRẠNG THÁI

Tài liệu sử dụng bốn nhãn:

- **[MVP–P0]**: Bắt buộc phải có trước khi phát hành MVP.
- **[MVP–P1]**: Có giá trị cao, chỉ làm sau khi P0 ổn định.
- **[SAU MVP]**: Đã định hướng nhưng không được chen vào làm chậm MVP.
- **[KHÔNG LÀM]**: Chủ động loại khỏi phạm vi hiện tại.

Quy tắc quản trị phạm vi:

- Không đưa một tính năng từ “Sau MVP” vào MVP nếu chưa loại một tính năng có khối lượng tương đương.
- Không gọi một chức năng là “đã làm” nếu mới có giao diện nhưng chưa có dữ liệu thật, engine thật, log thật và kiểm thử thật.
- Không coi build thành công là kiểm thử nghiệp vụ thành công.
- Không tự động hóa hành động có khả năng làm mất hoặc làm đứt đường dẫn file trước khi chế độ thủ công đã ổn định.

---

## 2. BÀI TOÁN THỊ TRƯỜNG

### 2.1. Vấn đề người dùng

Người dùng phổ thông thường có dữ liệu rải rác ở Downloads, Desktop, Documents, nhiều ổ đĩa, USB, ổ cứng cũ và thư mục đồng bộ cloud. Sau vài năm, họ gặp các vấn đề:

- Không biết file nào còn cần.
- Không biết bản nào là bản mới nhất.
- Có nhiều bản trùng nhưng không dám xóa.
- Dọn thủ công tốn thời gian và dễ sai.
- Phần mềm “cleaner” thường chỉ xóa cache, không hiểu kho file cá nhân.
- Công cụ tìm kiếm chỉ tìm khi người dùng biết cần tìm gì.
- Công cụ duplicate thường chỉ giải quyết một lát cắt nhỏ.
- Ứng dụng tự động di chuyển file dễ phá cấu trúc project và đường dẫn tham chiếu.

### 2.2. Khoảng trống sản phẩm

Khoảng trống không nằm ở việc thiếu thêm một file manager. Khoảng trống nằm ở một lớp “trí tuệ vận hành file” giữa người dùng và hệ thống file:

- Nhìn thấy toàn cảnh.
- Hiểu rủi ro trước khi hành động.
- Giải thích được đề xuất.
- Thực thi có giao dịch và có thể phục hồi.
- Học quy tắc người dùng nhưng không chiếm quyền quyết định.

### 2.3. Định vị

MH FileOS đứng giữa bốn nhóm sản phẩm:

| Nhóm hiện tại | Làm tốt | MH FileOS bổ sung |
|---|---|---|
| File Explorer/Everything | Duyệt và tìm file | Phân tích, lập kế hoạch, tổ chức và phục hồi |
| Cleaner | Xóa cache/rác hệ thống | Hiểu dữ liệu cá nhân, không xóa mù |
| Duplicate finder | Tìm file trùng | Đưa trùng lặp vào bức tranh tổ chức tổng thể |
| Hazel/DropIt/Mouzi | Luật tự động cho file mới | Quét kho cũ, bảo vệ vùng nguy hiểm, giao dịch và phục hồi |

---

## 3. NGƯỜI DÙNG MỤC TIÊU

### Persona A — Người dùng phổ thông nhiều năm không dọn máy

- Có 100.000–500.000 file.
- Sợ xóa nhầm.
- Không hiểu hash, metadata hay junction.
- Muốn biết “cái nào an toàn để xử lý”.

**Giá trị phải nhận được:** Nhìn thấy dung lượng, trùng lặp, file lớn và đề xuất dễ hiểu trong vài phút đầu.

### Persona B — Người sáng tạo nội dung

- Có ảnh, video, âm thanh, project và asset liên kết.
- Thường có nhiều phiên bản.
- Việc di chuyển sai một file có thể làm hỏng project.

**Giá trị phải nhận được:** Tổ chức được dữ liệu nhưng các vùng project được bảo vệ mặc định.

### Persona C — Người tải nhiều tài liệu và bộ cài

- Downloads nhanh chóng trở thành kho hỗn hợp.
- Cần luật tự động sau khi đã tin tưởng ứng dụng.

**Giá trị phải nhận được:** Chế độ Inbox giúp file mới luôn vào đúng nơi.

### Persona D — Người dùng kỹ thuật/power user

- Muốn regex, rule priority, include/exclude, export plan và log chi tiết.
- Muốn biết chính xác ứng dụng làm gì.

**Giá trị phải nhận được:** Luật mạnh, log máy đọc được, không có “AI bí ẩn”.

### Persona ưu tiên MVP

MVP ưu tiên Persona A và B. Persona C được hỗ trợ ở mức “Suggest Mode”. Persona D có đủ nền tảng để tin tưởng nhưng chưa cần toàn bộ công cụ nâng cao.

---

## 4. NGUYÊN TẮC THIẾT KẾ KHÔNG ĐƯỢC PHÁ VỠ

### 4.1. Local-first

- Không yêu cầu tài khoản.
- Không yêu cầu API key.
- Không upload tên file, đường dẫn, hash hoặc nội dung.
- Không có telemetry mặc định.
- Chẩn đoán lỗi chỉ xuất thành gói đã che dữ liệu nhạy cảm khi người dùng chủ động tạo.

### 4.2. Quét rộng, hành động hẹp

Ứng dụng có thể được cấp quyền đọc một phạm vi lớn để phân tích, nhưng quyền ghi chỉ được mở cho những mục trong một kế hoạch đã duyệt.

### 4.3. Kế hoạch trước, thao tác sau

Mọi thay đổi phải đi qua pipeline:

`Phát hiện → Đề xuất → Kế hoạch → Duyệt → Thực thi → Xác minh → Ghi nhận`

### 4.4. Không ghi đè mặc định

Nếu đích đã có file cùng tên:

- Mặc định: dừng mục đó và yêu cầu quyết định.
- Cho phép: bỏ qua, đổi tên an toàn, so sánh nội dung.
- “Ghi đè” chỉ xuất hiện khi người dùng mở tùy chọn nâng cao và xác nhận riêng.

### 4.5. Undo là một cam kết kỹ thuật

Không được hiển thị “Đã hoàn tác” nếu thao tác phục hồi chưa được kiểm chứng. Undo phải hoạt động cả cùng ổ và khác ổ.

### 4.6. Không đánh đồng giống tên với giống nội dung

- File cùng tên chưa chắc trùng.
- File khác tên vẫn có thể trùng.
- Xóa bản trùng chỉ được đề xuất sau full hash; trước khi xóa vĩnh viễn phải so khớp kích thước và tùy chế độ có thể byte-compare.

### 4.7. Mọi quyết định phải giải thích được

Ví dụ:

> “Đề xuất chuyển `invoice_2026.pdf` vào `Documents/Invoices/2026` vì đuôi PDF, tên chứa `invoice` và ngày tạo là năm 2026.”

### 4.8. Core không phụ thuộc AI

AI có thể hỗ trợ gắn nhãn sau MVP nhưng không được chặn tính năng cốt lõi, không được là nguồn quyết định xóa file và không được gửi dữ liệu ra ngoài nếu chưa có opt-in rõ ràng.

---

## 5. RANH GIỚI SẢN PHẨM

### 5.1. MVP sẽ làm

- Chọn một hoặc nhiều thư mục/ổ đĩa để quét.
- Quét đệ quy có kiểm soát.
- Lập chỉ mục metadata.
- Phân loại theo loại file.
- Hiển thị dung lượng theo loại/thư mục/thời gian.
- Tìm file rỗng và thư mục rỗng.
- Tìm file lớn.
- Tìm file trùng chính xác bằng pipeline hash nhiều tầng.
- Tạo kế hoạch tổ chức bằng rule.
- Preview trước/sau.
- Di chuyển/đổi tên có xác minh.
- Cách ly thay vì xóa trực tiếp.
- Hoàn tác chắc chắn.
- Vùng bảo vệ và ignore rules.
- Lịch sử đầy đủ.
- Giao diện tiếng Việt và tiếng Anh.
- Suggest Mode cho thư mục Inbox.

### 5.2. MVP chưa làm

- Phát hiện ảnh “gần giống” bằng perceptual hash.
- Nhận diện nội dung ảnh bằng AI.
- Tự học hành vi bằng mô hình ML.
- Phân tích nội dung tài liệu sâu.
- Parse toàn bộ định dạng project chuyên ngành.
- Tự dọn Windows Registry.
- Tối ưu RAM hoặc tăng tốc máy bằng tuyên bố marketing.
- Đồng bộ cloud riêng.
- App mobile.
- macOS/Linux trong bản phát hành đầu tiên.

### 5.3. Tuyệt đối không làm trong MVP

- Nút “Dọn toàn bộ” không có preview.
- Xóa vĩnh viễn hàng loạt theo điểm tin cậy AI.
- Quét/ghi tự do vào Windows, Program Files hoặc AppData.
- Tắt Defender, bypass quyền hệ điều hành hoặc yêu cầu admin không cần thiết.
- Thay đổi file khi scan read-only.

---

## 6. BỘ TÍNH NĂNG MVP THEO MỨC ƯU TIÊN

## 6.1. [MVP–P0] Onboarding an toàn

Luồng lần đầu:

1. Giải thích ứng dụng đang ở chế độ read-only.
2. Cho chọn thư mục đầu tiên; đề xuất Downloads hoặc một thư mục test.
3. Hiển thị vùng bị bảo vệ mặc định.
4. Cho chọn loại quét: Nhanh hoặc Đầy đủ.
5. Chạy scan và hiển thị tiến trình thật.
6. Mở trang kết quả, chưa tự thay đổi file.

Tiêu chí nghiệm thu:

- Không có thao tác ghi trong onboarding.
- Người dùng biết ứng dụng chưa xóa hay di chuyển gì.
- Có nút dừng scan.
- Có thể tiếp tục scan sau khi đóng app nếu checkpoint hợp lệ.

## 6.2. [MVP–P0] Scan Engine

Khả năng:

- Một hoặc nhiều scan root.
- Recursive scan.
- Bỏ qua symbolic link/junction/reparse point mặc định.
- Tùy chọn đi theo link chỉ dành cho nâng cao và phải chống vòng lặp.
- Hỗ trợ path dài của Windows.
- Hỗ trợ ký tự Unicode, dấu tiếng Việt, emoji, dấu nháy và ký tự hợp lệ đặc biệt.
- Không mở nội dung file khi chỉ cần metadata.
- Hàng đợi có backpressure; không tạo một task cho mỗi file vô hạn.
- Có cancellation token.
- Có checkpoint theo batch.
- Ghi nhận file không đọc được thay vì làm hỏng cả phiên scan.

Hai chế độ:

**Quick Scan**

- Path, tên, extension, kích thước.
- Created/modified/accessed nếu hệ thống cung cấp đáng tin.
- Thuộc tính hidden/system/read-only.
- File identity của Windows nếu lấy được.
- Không full hash toàn bộ.

**Deep Scan**

- Bao gồm Quick Scan.
- MIME sniffing có giới hạn.
- Quick fingerprint cho nhóm có khả năng trùng.
- Full hash cho ứng viên duplicate.
- Metadata adapter theo loại file được hỗ trợ.

## 6.3. [MVP–P0] File Catalog

Catalog là nguồn sự thật của giao diện, không phải hệ thống file trực tiếp.

Mỗi file cần lưu tối thiểu:

- ID nội bộ.
- Scan root và volume.
- Canonical path và display path.
- Tên file, extension, parent directory.
- Kích thước.
- Timestamp.
- Thuộc tính file.
- Windows file identity nếu có.
- Trạng thái tồn tại lần cuối.
- Quick fingerprint/full hash nếu đã tính.
- Category và confidence.
- Protected status.
- Lý do bị bỏ qua hoặc lỗi.

Catalog phải xử lý được:

- File bị đổi tên bên ngoài ứng dụng.
- File bị xóa giữa lúc scan.
- File thay đổi nội dung sau khi hash.
- Hai path trỏ đến cùng hardlink.
- Volume bị tháo ra.

## 6.4. [MVP–P0] Dashboard kết quả

Hiển thị:

- Tổng số file/thư mục.
- Tổng dung lượng đã lập chỉ mục.
- Phân bố theo Audio, Video, Image, Document, Archive, Installer, Project, Other.
- Top thư mục chiếm dung lượng.
- Top file lớn.
- Duplicate potential và duplicate confirmed.
- File rỗng/thư mục rỗng.
- File không truy cập được.
- Mức dung lượng có thể thu hồi, luôn ghi rõ “ước tính” hay “đã xác nhận”.

Yêu cầu UX:

- Không làm dashboard kiểu trang trí.
- Mỗi con số bấm được để mở danh sách nguồn.
- Mỗi danh sách hỗ trợ sort, filter, search, select.
- Danh sách phải virtualize, không render hàng trăm nghìn row.

## 6.5. [MVP–P0] Exact Duplicate Finder

Pipeline để giảm I/O:

1. Nhóm theo kích thước.
2. Bỏ nhóm chỉ có một file.
3. Tính quick fingerprint ở đầu, giữa và cuối file.
4. Chỉ full hash những file còn trùng quick fingerprint.
5. Nhóm theo full BLAKE3 hoặc SHA-256.
6. Trước hành động loại bỏ, kiểm tra lại size, mtime và hash.
7. Chế độ độ chắc chắn tối đa thực hiện byte-by-byte compare.

Không được:

- Gọi file trùng chỉ vì cùng tên.
- Tự chọn bản xóa mà không giải thích.
- Chọn xóa tất cả bản trong một nhóm.

Quy tắc đề xuất bản giữ:

- Ưu tiên file trong vùng được đánh dấu “Original/Project/Protected”.
- Ưu tiên path ngắn, cấu trúc rõ ràng.
- Ưu tiên file có timestamp hợp lệ.
- Không ưu tiên theo “mới hơn” nếu nội dung hoàn toàn giống mà không giải thích.
- Người dùng luôn có thể đổi bản giữ.

Hành động MVP:

- Giữ tất cả.
- Đưa bản thừa vào Quarantine.
- Mở vị trí.
- So sánh metadata.
- Chọn thủ công bản giữ.

## 6.6. [MVP–P0] Large Files & Space Analysis

- Ngưỡng có thể chỉnh: 100 MB, 500 MB, 1 GB, custom.
- Hiển thị tỷ trọng so với thư mục và volume.
- Nhận diện installer/archive cũ ở mức đề xuất, không tự xóa.
- Cho đánh dấu “Keep” để không đề xuất lại.
- Cho mở Explorer, xem properties và thêm vào plan.

## 6.7. [MVP–P0] Rule Engine

Điều kiện hỗ trợ MVP:

- Extension.
- Tên file chứa/bắt đầu/kết thúc.
- Regex.
- Kích thước.
- Ngày tạo/sửa.
- Category.
- Scan root hoặc thư mục nguồn.
- Depth tương đối.
- Có/không có thuộc tính hidden/system/read-only.

Hành động hỗ trợ MVP:

- Đề xuất move.
- Đề xuất rename.
- Bỏ qua.
- Gắn nhãn nội bộ.
- Đưa vào Quarantine.

Placeholders:

- `{year}`, `{month}`, `{day}`.
- `{extension}`.
- `{filename}`, `{stem}`.
- `{category}`.
- `{source_root}`.

Quy tắc bắt buộc:

- Rule thuộc một Rule Set.
- Rule Set gắn rõ với scan root/inbox; không dùng toàn cục mơ hồ.
- Priority xác định và có màn hình mô phỏng.
- First-match hoặc continue phải khai báo rõ.
- Regex được compile khi lưu, lỗi phải trả ngay.
- Destination được chuẩn hóa và kiểm tra có thoát khỏi vùng cho phép hay không.

## 6.8. [MVP–P0] Plan Builder — Trái tim sản phẩm

Mọi hành động phải được chuyển thành `Action Plan` bất biến theo phiên bản.

Plan chứa:

- Plan ID, created time, source scan snapshot.
- Danh sách item.
- Source path và source fingerprint.
- Proposed destination.
- Operation type.
- Rule/reason tạo đề xuất.
- Conflict status.
- Risk level.
- Estimated bytes moved/freed.
- Undo feasibility.

Trước khi cho chạy, hệ thống phải validate:

- Source vẫn tồn tại.
- Source chưa đổi từ snapshot.
- Destination hợp lệ.
- Dung lượng đích đủ.
- Không có path collision chưa giải quyết.
- Không đi vào vùng bảo vệ.
- Không tạo vòng lặp thư mục.
- Không move thư mục vào chính nó.
- Không thao tác file đang bị lock khi chế độ lock check bật.

Preview phải hiển thị:

- Trước → Sau.
- Lý do.
- Mức rủi ro.
- File bị bỏ qua và lý do.
- Thay đổi dung lượng theo volume.

## 6.9. [MVP–P0] Transactional File Executor

Đây là phần có tiêu chuẩn chất lượng cao nhất.

### Cùng volume

1. Ghi `operation_intent` vào journal.
2. Revalidate source.
3. Tạo destination directory nếu được phép.
4. Atomic rename khi hệ thống file hỗ trợ.
5. Kiểm tra destination tồn tại và identity phù hợp.
6. Ghi `committed`.

### Khác volume

1. Ghi intent.
2. Kiểm tra free space.
3. Copy vào file tạm tại destination.
4. Flush và đóng file.
5. Xác minh size và full hash; chế độ nghiêm ngặt có byte compare.
6. Đổi tên file tạm thành tên đích.
7. Đưa source vào Quarantine hoặc xóa source theo chính sách đã duyệt.
8. Kiểm tra trạng thái cuối.
9. Ghi committed.

### Trạng thái operation

`planned → validated → running → copied → verified → source_retired → committed`

Trạng thái lỗi:

- `blocked`
- `failed_recoverable`
- `failed_attention_required`
- `rolled_back`

Ứng dụng crash ở bất kỳ trạng thái nào cũng phải đọc journal khi khởi động lại và đưa ra lựa chọn Resume/Rollback/Inspect.

## 6.10. [MVP–P0] Quarantine

Quarantine là vùng trung gian do ứng dụng quản lý.

- Không dùng một thư mục bí mật không giải thích.
- Hiển thị path gốc, lý do, thời gian, kích thước và ngày tự xóa dự kiến.
- Mặc định không tự xóa trong MVP.
- Cho Restore từng file/nhóm.
- Quarantine trên cùng volume khi có thể để thao tác nhanh.
- Nếu volume rời, trạng thái phải là unavailable chứ không báo mất.

## 6.11. [MVP–P0] Undo/Recovery

Undo phải dựa trên journal, không dựa riêng vào dòng history hiển thị.

Khi undo:

1. Kiểm tra destination hiện tại.
2. Xác minh nó đúng file đã thao tác.
3. Kiểm tra source path cũ có bị chiếm không.
4. Nếu conflict, yêu cầu lựa chọn.
5. Thực hiện reverse transaction.
6. Xác minh kết quả.
7. Chỉ sau đó mới ghi `undo_committed`.

Yêu cầu:

- Undo cùng volume.
- Undo khác volume.
- Undo sau restart.
- Undo một item.
- Undo một plan theo thứ tự ngược.
- Không đánh dấu thành công khi file thiếu hoặc rename/copy thất bại.

## 6.12. [MVP–P0] Protected Zones

Danh sách bảo vệ mặc định Windows:

- `C:\Windows`
- `C:\Program Files`
- `C:\Program Files (x86)`
- `C:\ProgramData`
- System Volume Information
- Recycle Bin internals
- AppData ở chế độ write
- Thư mục ứng dụng đang chạy và database của MH FileOS

Phát hiện vùng nhạy cảm:

- Git repository.
- Node modules/venv/cache build.
- OneDrive/Dropbox/Google Drive sync root.
- Thư mục có project DAW, video editor, IDE project.
- Thư mục có file database đang dùng.
- Junction/reparse point.
- Network share.
- USB/removable volume.

Mức bảo vệ:

- `BLOCKED`: Không cho thao tác.
- `READ_ONLY`: Quét được, không tạo plan ghi.
- `CONFIRM_EACH_PLAN`: Mỗi plan phải xác nhận nâng cao.
- `NORMAL`: Cho phép theo policy chung.

Đối với FL Studio trong MVP:

- Nếu thư mục chứa `.flp`, đánh dấu là Project Zone.
- Audio dưới Project Zone không được tự động đề xuất di chuyển.
- Có thể phân tích dung lượng/trùng lặp nhưng duplicate action mặc định là Keep.
- Parse dependency `.flp` là [SAU MVP], không được giả vờ đã hỗ trợ.

## 6.13. [MVP–P1] Inbox Suggest Mode

Đây là phần học từ Mouzi nhưng được triển khai an toàn hơn.

- Theo dõi Downloads hoặc thư mục Inbox do người dùng chọn.
- File mới được phát hiện và chờ ổn định.
- Rule engine tạo đề xuất.
- Thông báo: “5 file sẵn sàng được tổ chức”.
- Người dùng mở preview và duyệt.
- Sau khi một rule được duyệt thành công nhiều lần, có thể cho phép Auto Mode riêng rule đó.

Không bật Auto Mode mặc định trong MVP.

## 6.14. [MVP–P1] Empty Files & Empty Folders

- File 0 byte không mặc định là rác.
- Phân biệt placeholder cloud, lock file, marker file.
- Thư mục rỗng trong protected/project zone không được đề xuất xóa.
- Hành động mặc định là Quarantine hoặc bỏ qua.

---

## 7. KIẾN TRÚC KỸ THUẬT

## 7.1. Quyết định stack

### Desktop shell

- **Tauri 2** cho cửa sổ desktop và đóng gói.
- **React + TypeScript** cho giao diện.
- Không nhúng website từ xa vào WebView.
- CSP phải được cấu hình, không để `null`.

### Core engine

- **Rust** làm scanner, catalog, hashing, rules, planning, executor, journal, watcher.
- Core tách khỏi Tauri để unit/integration test không cần UI.
- Giao tiếp qua command/query và event typed rõ ràng.

### Storage

- **SQLite** với WAL.
- Migration có version.
- Transaction cho thay đổi database.
- FTS5 cho tìm kiếm tên/path nếu cần.
- Không lưu nội dung file vào database.

### Lý do chọn

- Rust phù hợp I/O song song và an toàn bộ nhớ.
- Tauri cho binary nhỏ hơn Electron và tận dụng WebView hệ thống.
- SQLite đủ cho local catalog hàng triệu bản ghi nếu index/query đúng.
- React giúp phát triển UI nhanh nhưng phải dùng virtualization.

## 7.2. Cấu trúc repository đề xuất

```text
mh-fileos/
├─ apps/
│  └─ desktop/                 # Tauri + React shell
├─ crates/
│  ├─ fileos-domain/           # Entity, value object, state machine
│  ├─ fileos-scanner/          # Walk, metadata, cancellation
│  ├─ fileos-catalog/          # SQLite repository, migrations
│  ├─ fileos-fingerprint/      # Quick/full hash
│  ├─ fileos-classifier/       # Deterministic categories
│  ├─ fileos-rules/            # Rule compile/evaluate/simulate
│  ├─ fileos-planner/          # Build/validate immutable plans
│  ├─ fileos-executor/         # Transactional file operations
│  ├─ fileos-journal/          # Append-only operation journal
│  ├─ fileos-watcher/          # Inbox suggest mode
│  ├─ fileos-platform-windows/ # File ID, lock, recycle, volume info
│  └─ fileos-testkit/          # Temp FS, fixtures, fault injection
├─ packages/
│  ├─ ui/                      # Shared UI components
│  └─ contracts/               # Generated TS types/events
├─ docs/
│  ├─ product/
│  ├─ architecture/
│  ├─ adr/
│  ├─ testing/
│  └─ threat-model/
├─ fixtures/
├─ scripts/
└─ .github/workflows/
```

Không đặt logic file operation trong React component hoặc Tauri command mỏng. Tauri command chỉ validate DTO, gọi use case và trả kết quả typed.

## 7.3. Các bounded context

### Catalog

Trả lời “máy có gì” dựa trên snapshot.

### Analysis

Tạo findings: duplicate, large, empty, category, inaccessible.

### Planning

Biến findings/rules thành plan nhưng chưa đụng file.

### Execution

Chỉ nhận plan đã validate và đã duyệt.

### Recovery

Đọc journal và phục hồi state sau crash.

### Automation

Theo dõi file mới và tạo suggestion; không được bypass Planning/Execution.

---

## 8. MÔ HÌNH DỮ LIỆU

## 8.1. Bảng chính

### `volumes`

- `id`
- `volume_guid`
- `serial_number`
- `mount_point`
- `fs_type`
- `is_removable`
- `is_network`
- `last_seen_at`

### `scan_roots`

- `id`
- `volume_id`
- `path`
- `canonical_path`
- `mode`
- `protection_level`
- `include_hidden`
- `follow_reparse_points`
- `created_at`

### `scan_runs`

- `id`
- `root_id`
- `kind`
- `state`
- `started_at`, `finished_at`
- `files_seen`, `dirs_seen`, `bytes_seen`
- `errors_count`
- `checkpoint`

### `file_entries`

- `id`
- `root_id`, `volume_id`
- `parent_id`
- `path_key`
- `display_path`
- `name`, `extension`
- `size_bytes`
- `created_at_fs`, `modified_at_fs`, `accessed_at_fs`
- `file_attributes`
- `platform_file_id`
- `is_directory`, `is_symlink`, `is_reparse_point`
- `category`, `category_confidence`
- `protection_level`
- `last_scan_run_id`
- `existence_state`

### `fingerprints`

- `file_entry_id`
- `size_snapshot`
- `mtime_snapshot`
- `quick_hash`
- `full_hash`
- `hash_algorithm`
- `verified_at`
- `stale`

### `findings`

- `id`
- `type`
- `severity`
- `confidence`
- `primary_file_id`
- `group_key`
- `evidence_json`
- `state`

### `rule_sets`, `rules`

- Scope rõ ràng theo root/inbox.
- Priority.
- Compiled condition version.
- Action template.
- Enabled/version/timestamps.

### `action_plans`, `plan_items`

- Plan snapshot và trạng thái duyệt.
- Source/destination.
- Expected fingerprint.
- Risk và reason.
- Conflict resolution.

### `operations`, `operation_events`

- State hiện tại.
- Append-only events.
- Error code có cấu trúc.
- Before/after identity.
- Undo linkage.

### `protected_zones`, `ignore_patterns`

- Scope.
- Pattern/path.
- Level.
- Origin: system, detected, user.
- Không cho user vô tình xóa protected rule hệ thống mà không có cảnh báo.

## 8.2. Quy tắc database

- Mọi migration được test trên DB cũ.
- Không `unwrap()` dữ liệu timestamp từ DB.
- Không giữ mutex database toàn cục trong thao tác I/O dài.
- Dùng connection pool có giới hạn.
- Index theo `root_id`, `path_key`, `size_bytes`, `full_hash`, `category`, `modified_at_fs`.
- Batch insert/update.
- Catalog snapshot và operation journal không được trộn trách nhiệm.

---

## 9. THIẾT KẾ SCANNER VÀ HIỆU NĂNG

## 9.1. Pipeline

1. Enumerator đọc directory entries.
2. Metadata workers đọc metadata.
3. Protection classifier đánh dấu vùng.
4. Catalog writer ghi batch.
5. Fingerprint scheduler nhận ứng viên, không hash mọi file ngay.
6. UI event aggregator phát tiến trình tối đa vài lần/giây.

## 9.2. Backpressure

- Channel bounded.
- Số worker dựa trên loại volume.
- HDD dùng concurrency thấp để tránh seek storm.
- SSD có thể cao hơn nhưng vẫn có giới hạn.
- Network/removable mặc định conservative.

## 9.3. Mục tiêu hiệu năng MVP

Các mục tiêu này phải đo bằng benchmark fixture, không dùng làm quảng cáo trước khi có số thật:

- App mở tới màn hình chính: mục tiêu dưới 2 giây trên máy tham chiếu.
- UI interaction: dưới 100 ms cho filter/sort đã index.
- Idle CPU: trung bình dưới 1% khi không watch/scan.
- Idle memory: mục tiêu dưới 200 MB; không tuyên bố 5 MB phi thực tế khi dùng WebView.
- Quick scan 100.000 file trên SSD: mục tiêu dưới 3 phút, không full hash.
- Catalog 1.000.000 file vẫn mở/filter được nhờ pagination/virtualization.
- Progress không được làm UI lag.

Máy benchmark tham chiếu phải được ghi rõ CPU, RAM, ổ đĩa, số file và trạng thái cache.

## 9.4. Cache invalidation

Fingerprint bị stale nếu:

- Size đổi.
- Mtime đổi.
- File identity đổi.
- Volume serial khác.

Không sử dụng hash cũ để ra quyết định destructive nếu snapshot không còn đúng.

---

## 10. UI/UX CHI TIẾT

## 10.1. Nguyên tắc giao diện

- Tiếng Việt là ngôn ngữ cấp một, không phải bản dịch phụ.
- Nền sáng sạch, chữ rõ, không xanh đậm nặng nề.
- Không dùng dashboard giả nhiều card nhưng ít hành động.
- Mọi kết quả đều dẫn đến danh sách bằng chứng.
- Cảnh báo dùng ngôn ngữ con người, không ném error Rust/SQL ra UI.
- Chế độ nâng cao không làm rối người mới.

## 10.2. Navigation MVP

1. **Tổng quan**
2. **Quét dữ liệu**
3. **Kết quả**
4. **File trùng**
5. **Kế hoạch tổ chức**
6. **Luật**
7. **Lịch sử & Phục hồi**
8. **Vùng bảo vệ**
9. **Cài đặt**

## 10.3. Màn hình Tổng quan

- Nút chính: “Bắt đầu quét”.
- Scan gần nhất và độ mới dữ liệu.
- Dung lượng theo category.
- Vấn đề cần chú ý.
- Kế hoạch đang chờ duyệt.
- Operation chưa hoàn tất sau crash phải nổi lên đầu tiên.

## 10.4. Màn hình Quét

- Chọn root bằng dialog thật, không bắt nhập path thủ công là cách chính.
- Cho xem quyền và protection trước khi quét.
- Toggle hidden/system có giải thích.
- Quick/Deep.
- Tiến trình: path hiện tại, file, dung lượng, lỗi, tốc độ.
- Pause/Resume/Cancel.

## 10.5. Kết quả

Mỗi row có:

- Tên, path, size, modified date, category.
- Badge protected/risk.
- Lý do finding.
- Open/Reveal.
- Add to plan.

Bulk action chỉ bật khi các item tương thích.

## 10.6. File trùng

- Nhóm từng cụm duplicate.
- Highlight bản đề xuất giữ.
- So sánh path, timestamps, protected status.
- Không preselect bản xóa khi confidence chưa confirmed.
- Tổng dung lượng có thể thu hồi được cập nhật theo lựa chọn.

## 10.7. Plan Review

- Bảng Before/After.
- Filter theo risk/conflict/rule.
- Mô phỏng cây thư mục sau thay đổi.
- “Validate lại” trước Execute.
- Checkbox xác nhận cho high-risk item.
- Export plan JSON/CSV cho power user.

## 10.8. Execution Progress

- Hiển thị item hiện tại, bytes, state.
- Pause chỉ ở điểm an toàn.
- Cancel chuyển sang rollback/stop-after-current, không kill giữa copy.
- Lỗi một item không làm mất log các item khác.

## 10.9. History & Recovery

- Nhóm theo plan.
- Trạng thái trung thực.
- Undo khả dụng hay không và lý do.
- Nếu source/destination thay đổi bên ngoài, hiển thị “Cần kiểm tra”, không giả vờ undo.

---

## 11. CONTRACTS VÀ ERROR MODEL

## 11.1. Command/query tách biệt

Query không được thay đổi file:

- `start_scan`
- `get_scan_status`
- `list_files`
- `list_findings`
- `simulate_rule_set`
- `build_plan`
- `validate_plan`

Command thay đổi trạng thái:

- `approve_plan`
- `execute_plan`
- `pause_execution`
- `resume_execution`
- `undo_operation`
- `restore_quarantine_item`

## 11.2. Error có cấu trúc

Ví dụ:

```json
{
  "code": "DESTINATION_CONFLICT",
  "message": "Vị trí đích đã có file cùng tên.",
  "operationId": "op_...",
  "source": "...",
  "destination": "...",
  "recoverable": true,
  "suggestedActions": ["skip", "rename", "compare"]
}
```

Không dựa vào parse chuỗi lỗi để điều khiển UI.

---

## 12. SECURITY & THREAT MODEL

## 12.1. Các mối đe dọa chính

- Path traversal.
- Command injection khi mở Explorer.
- Symlink/junction escape.
- TOCTOU: file thay đổi giữa validate và execute.
- Archive bomb.
- Malicious filename.
- Database corruption.
- Privilege escalation không cần thiết.
- WebView XSS gọi command native.
- Tampered update/installer.

## 12.2. Biện pháp

- Không dựng chuỗi PowerShell/cmd từ path.
- Dùng Windows API hoặc opener API với argument typed.
- CSP chặt; không load remote script.
- Tauri permissions theo cửa sổ và capability tối thiểu.
- Validate canonical destination nằm trong scope được phép.
- Không follow reparse point mặc định.
- Mỗi operation revalidate file identity ngay trước thay đổi.
- Archive import sau MVP phải có giới hạn số entry, tổng kích thước, compression ratio và path safety.
- Database backup/quick integrity check.
- Installer phải ký code trước public launch nếu ngân sách cho phép.
- Publish SHA-256 checksums và provenance build.
- Dependency audit và SBOM.

## 12.3. Quyền hệ điều hành

- App chạy non-admin mặc định.
- Nếu root cần quyền cao, giải thích chính xác thao tác nào cần quyền.
- Không chạy toàn bộ app dưới admin chỉ để xử lý một thư mục.

---

## 13. CHIẾN LƯỢC KIỂM THỬ

## 13.1. Unit tests

- Path normalization.
- Unicode/case sensitivity.
- Rule matching và priority.
- Placeholder resolution.
- Ignore pattern.
- Duplicate grouping.
- Plan validation.
- Operation state machine.
- Migration.

## 13.2. Property-based tests

Sinh ngẫu nhiên:

- Tên file lạ.
- Path sâu.
- Conflict combinations.
- Rule combinations.
- Thứ tự crash point.

Invariant:

- Không có hai plan item ghi đè cùng destination nếu chưa resolve.
- Không mất cả source lẫn destination.
- Committed move luôn có đúng một bản ở vị trí cuối theo policy.
- Undo committed phục hồi trạng thái kỳ vọng hoặc báo conflict trung thực.

## 13.3. Integration tests với filesystem thật

- Cùng volume.
- Khác volume.
- File locked.
- Read-only file.
- Destination hết dung lượng giả lập.
- Permission denied.
- Network disconnect.
- USB removal.
- Long path.
- Unicode/emoji/apostrophe/ampersand.
- Junction loop.
- OneDrive placeholder.

## 13.4. Fault injection

Buộc crash tại từng điểm:

- Trước copy.
- Giữa copy.
- Sau copy trước verify.
- Sau verify trước retire source.
- Sau retire source trước commit DB.
- Giữa undo.

Sau restart phải có kết quả xác định, không để trạng thái mơ hồ.

## 13.5. UI tests

- Onboarding.
- Scan cancel/resume.
- Filter 100.000 row qua virtualized list.
- Duplicate selection không cho xóa tất cả bản.
- Conflict resolution.
- Plan approval.
- Recovery prompt.
- Tiếng Việt không tràn layout.

## 13.6. Performance tests

Fixture:

- 10k file nhỏ.
- 100k mixed.
- 1M metadata-only.
- 100 GB duplicate candidates.
- HDD vs SSD.

Đo:

- Throughput.
- CPU/RAM.
- DB growth.
- UI frame time.
- Hash I/O.
- Time to cancel.

## 13.7. Test dữ liệu thật

Ba vòng:

1. Synthetic fixture.
2. Bản sao dữ liệu thật đã backup.
3. Beta opt-in với chế độ Suggest Only.

Không beta Auto Mode trước khi vòng 2 không có lỗi dữ liệu.

---

## 14. CI/CD VÀ CHẤT LƯỢNG PHÁT HÀNH

Mỗi PR phải chạy:

1. Rust format.
2. Clippy với warning nghiêm ngặt.
3. Rust unit/integration tests.
4. Frontend typecheck.
5. ESLint.
6. Frontend unit tests.
7. Contract generation consistency.
8. Security/dependency audit.
9. Build Windows debug artifact.

Nightly:

- Fault-injection suite.
- 100k/1M performance fixture.
- Database migration matrix.
- Windows 10/11 matrix nếu hạ tầng cho phép.

Release:

- Tag từ commit đã qua gate.
- Reproducible build ở mức khả thi.
- Signed installer.
- SHA-256 checksum.
- SBOM.
- Changelog thật.
- Smoke test clean VM.
- Defender/SmartScreen validation.

Không refresh binary “im lặng” dưới cùng version. Mọi thay đổi binary phải tăng build/version để người dùng kiểm chứng được.

---

## 15. LỘ TRÌNH TỪ SỐ 0

Ước lượng dành cho một kỹ sư chính làm toàn thời gian, có AI hỗ trợ nhưng vẫn tự review và test. Đây là sản phẩm can thiệp file thật; MVP đáng tin cậy không phải dự án cuối tuần.

### Giai đoạn 0 — Product & Safety Contract (Tuần 1)

Deliverables:

- Product brief.
- Safety invariants.
- Threat model v1.
- ADR chọn Rust/Tauri/SQLite.
- Repository scaffold.
- CI cơ bản.
- Fixture generator.

Exit gate:

- Các hành động P0 và non-goal được chốt.
- Không bắt đầu UI đẹp trước khi state machine operation được viết thành tài liệu.

### Giai đoạn 1 — Read-only Scanner & Catalog (Tuần 2–3)

Deliverables:

- Scanner recursive.
- Cancellation/backpressure.
- SQLite schema/migration.
- Catalog batch writer.
- Progress event.
- CLI/dev harness để scan không cần UI.

Exit gate:

- Scan fixture 100k không crash.
- Cancel trong thời gian chấp nhận được.
- Không thay đổi file nguồn.

### Giai đoạn 2 — Desktop Shell & Results (Tuần 4)

Deliverables:

- Onboarding.
- Scan screen.
- Dashboard thật.
- Virtualized file table.
- Filter/sort/search.
- Việt/Anh.

Exit gate:

- Mọi số liệu UI truy được về danh sách nguồn.
- Không có placeholder/fake metric.

### Giai đoạn 3 — Analysis & Exact Duplicates (Tuần 5–6)

Deliverables:

- Category classifier.
- Large/empty findings.
- Multi-stage fingerprint.
- Duplicate groups.
- Duplicate review UI.

Exit gate:

- Không có false duplicate trong fixture.
- Hash stale được phát hiện.
- Không có delete thật.

### Giai đoạn 4 — Rules & Plan Builder (Tuần 7–8)

Deliverables:

- Scoped rule sets.
- Rule editor và simulator.
- Plan builder.
- Conflict detection.
- Before/after preview.
- Protected zones.

Exit gate:

- Plan deterministic.
- Source thay đổi sau scan làm plan invalid.
- Không thoát scope destination.

### Giai đoạn 5 — Transactional Executor (Tuần 9–10)

Deliverables:

- Same-volume move.
- Cross-volume copy/verify/retire.
- Journal.
- Quarantine.
- Progress/error model.

Exit gate:

- Fault injection qua mọi checkpoint.
- Không mất dữ liệu trong test matrix.
- Không overwrite im lặng.

### Giai đoạn 6 — Undo & Crash Recovery (Tuần 11)

Deliverables:

- Single/group undo.
- Cross-volume undo.
- Startup recovery.
- Conflict-aware restore.
- History UI.

Exit gate:

- 100% test operation đã committed có kết quả undo xác định.
- Không có false-success.

### Giai đoạn 7 — Suggest Mode & Polish (Tuần 12–13)

Deliverables:

- Inbox watcher.
- Stability/grace detection.
- Suggestions.
- Notifications.
- Accessibility và keyboard navigation.
- Performance optimization.

Exit gate:

- Watcher không tự move.
- File đang tải không bị đề xuất sớm.

### Giai đoạn 8 — Hardening & Private Beta (Tuần 14–16)

Deliverables:

- Security review.
- Clean VM test.
- Large data test.
- Beta feedback.
- Signed release candidate.
- User guide và recovery guide.

Exit gate:

- Không P0/P1 data-loss bug.
- Crash recovery pass.
- Installer reputation/scan chấp nhận được.
- Có rollback plan cho release.

### Ước lượng thực tế

- Prototype có hình dạng: 3–4 tuần.
- MVP nội bộ dùng được: 10–12 tuần.
- MVP đủ tin cậy để public: 14–16 tuần full-time.
- Nếu làm bán thời gian: 20–28 tuần.

AI có thể tăng tốc viết code và test fixture nhưng không rút ngắn được giai đoạn soak test, kiểm thử dữ liệu và xây dựng niềm tin.

---

## 16. BACKLOG ƯU TIÊN

### P0 — Không có thì không phát hành

- Scanner read-only.
- Catalog bền vững.
- Dashboard bằng dữ liệu thật.
- Exact duplicates.
- Protected zones.
- Rule scope rõ ràng.
- Immutable plan.
- Transaction executor.
- Verify cross-volume.
- Quarantine.
- Undo/recovery.
- Việt/Anh.
- CI tests.
- Installer/checksum.

### P1 — Hoàn thiện MVP

- Inbox Suggest Mode.
- Export/import rules.
- Export plan/report.
- Empty folders.
- Mark Keep/Ignore.
- Diagnostic bundle redacted.
- Scheduled read-only scan.

### Sau MVP 1.1

- Auto Mode theo từng rule đã được tin cậy.
- Similar images.
- Media metadata nâng cao.
- Duplicate video/audio nâng cao.
- Cloud-sync awareness sâu hơn.
- Rule templates marketplace không cần tài khoản.

### Sau MVP 1.5

- Project dependency adapters.
- FL Studio asset manifest/report.
- Adobe/DAW project protection plugin.
- Local semantic search.
- Local AI tagging tùy chọn.

### V2

- Linux/macOS.
- LAN/NAS catalog.
- Multi-device view không upload nội dung.
- Plugin SDK có sandbox.

---

## 17. RỦI RO VÀ GIẢM THIỂU

| Rủi ro | Mức | Giảm thiểu |
|---|---:|---|
| Mất dữ liệu khi move | Cực cao | Copy–verify–retire, journal, fault injection |
| Undo báo sai | Cực cao | State machine, verify, không bỏ qua error |
| Phá project/reference | Cao | Protected project zone, Suggest Only |
| False duplicate | Cao | Size → quick hash → full hash → optional byte compare |
| Scope creep | Cao | P0/P1/After MVP, change budget |
| Scan chậm | Trung bình | Backpressure, staged hash, volume-aware concurrency |
| Database quá lớn | Trung bình | Batch, index, retention, benchmark 1M |
| Defender cảnh báo | Cao | Code signing, reproducible CI, versioned binaries |
| Cloud sync conflict | Cao | Detect sync roots, conservative write policy |
| Người dùng không hiểu rủi ro | Cao | Explainable plan, risk badge, onboarding |
| AI làm quyết định sai | Loại bỏ MVP | Core deterministic, AI không destructive |

---

## 18. CHỈ SỐ THÀNH CÔNG

### Chỉ số chất lượng bắt buộc

- Zero confirmed data-loss incident trong test/beta.
- Undo success 100% trên operation được hỗ trợ trong matrix.
- Zero silent overwrite.
- Crash recovery deterministic.
- 100% destructive action có audit trail.

### Chỉ số trải nghiệm

- Người dùng hiểu kết quả đầu tiên trong dưới 5 phút với thư mục mẫu.
- Time to first useful finding dưới 5 phút trên scan root thông thường.
- Trên 70% beta user hoàn tất scan đầu tiên.
- Trên 50% beta user tạo ít nhất một plan.
- Tỷ lệ suggestion được chấp nhận dùng để cải thiện rule template, không để tự động hóa mù.

### Chỉ số hiệu năng

- Theo benchmark đã định ở mục 9.
- Không đặt chỉ số marketing trước khi có log benchmark tái lập.

---

## 19. ĐỊNH NGHĨA “DONE” CHO MỘT TÍNH NĂNG

Một tính năng chỉ được đánh dấu Done khi:

1. Có yêu cầu và acceptance criteria.
2. Có backend/core thật.
3. Có UI trạng thái loading/empty/error/success.
4. Có error model typed.
5. Có unit test.
6. Có integration test nếu đụng filesystem.
7. Có log/audit phù hợp.
8. Có tiếng Việt và tiếng Anh.
9. Có accessibility cơ bản.
10. Có tài liệu người dùng nếu hành vi không tự giải thích.
11. Đã test trên Windows 10 và 11 hoặc ghi rõ chưa test.
12. Không còn placeholder/TODO trong đường chạy production.

---

## 20. RELEASE GATE MVP

Không phát hành public nếu còn một trong các điều kiện:

- Có lỗi mất file hoặc false undo.
- Cross-volume chưa verify.
- Crash giữa operation không phục hồi được.
- Có thể overwrite không xác nhận.
- Protected zone có thể bị bypass bằng symlink/junction.
- Duplicate có thể xóa hết mọi bản.
- Binary không có version rõ ràng.
- Workflow chỉ build mà không test.
- Installer bị cảnh báo mà chưa có hướng xử lý an toàn.
- UI hiển thị metric không truy được nguồn.

MVP được phép phát hành khi:

- Toàn bộ P0 pass.
- Fault-injection suite pass.
- Clean VM Windows 10/11 pass.
- Private beta ít nhất 2 tuần không có P0/P1 data-integrity issue.
- Có tài liệu backup, recovery và giới hạn sản phẩm.
- Có checksum và release note.

---

## 21. CHIẾN LƯỢC SỬ DỤNG Ý TƯỞNG/MÃ NGUỒN MOUZI

Mouzi nên được dùng làm tài liệu tham khảo cho:

- Tray UX.
- Watcher.
- Rule concepts.
- Folder modes.
- Ignore file.
- Local SQLite history.

Không nên bê nguyên lõi vì:

- Thiết kế ban đầu ưu tiên một thư mục inbox.
- Rule scope chưa đủ cho hệ thống lớn.
- Undo và cross-volume cần transactional design mới.
- Scan không phải catalog toàn máy.
- Security boundary và recovery chưa đạt yêu cầu sản phẩm dữ liệu quan trọng.

Nếu tái sử dụng bất kỳ mã MIT nào:

- Giữ copyright và license notice theo MIT.
- Ghi rõ phần được sửa đổi.
- Ưu tiên viết core mới dựa trên specification để tránh kéo theo nợ kiến trúc.

---

## 22. QUYẾT ĐỊNH MVP CUỐI CÙNG

### Sản phẩm MVP được bán bằng lời hứa nào?

> “Quét kho file của bạn, chỉ ra thứ đang chiếm chỗ và lộn xộn, giúp bạn tổ chức hoặc loại bỏ bản trùng bằng một kế hoạch có thể kiểm tra và hoàn tác an toàn.”

### Tính năng khiến người dùng giữ ứng dụng

Không phải một dashboard đẹp. Đó là tổ hợp:

1. Scan toàn cảnh nhanh.
2. Duplicate chính xác.
3. Kế hoạch tổ chức dễ hiểu.
4. Thực thi không làm mất dữ liệu.
5. Undo đáng tin cậy.
6. Inbox Suggest Mode giúp máy không lộn xộn trở lại.

### Thứ tự ưu tiên tuyệt đối

1. **Không mất dữ liệu.**
2. **Kết quả đúng.**
3. **Giải thích rõ.**
4. **Hiệu năng tốt.**
5. **Giao diện đẹp.**
6. **Tự động hóa.**

Nếu phải hy sinh một tính năng để giữ an toàn và tính đúng, phải hy sinh tính năng.

---

## 23. 10 NHIỆM VỤ ĐẦU TIÊN ĐỂ BẮT ĐẦU CODE

1. Tạo mono-repo và cấu trúc crates/packages.
2. Viết `SAFETY-INVARIANTS.md` và state machine operation.
3. Thiết lập CI chạy format, lint, test và Windows build.
4. Tạo filesystem fixture generator.
5. Thiết kế SQLite migration v1.
6. Viết scanner CLI read-only có cancel/backpressure.
7. Viết catalog repository và benchmark 100k file.
8. Viết path/protected-zone module cho Windows.
9. Dựng Tauri shell chỉ hiển thị scan thật.
10. Chỉ sau khi tám phần trên pass mới xây dashboard và duplicate pipeline.

Không bắt đầu bằng trang landing, animation, AI tagging hoặc Auto Mode.

---

## 24. CHECKPOINT DUYỆT TRƯỚC KHI TRIỂN KHAI

Các quyết định đã tạm khóa trong tài liệu:

- Windows 10/11 trước.
- Rust + Tauri + React + SQLite.
- Local-first, không API bắt buộc.
- Scan rộng nhưng write có scope.
- Suggest trước, Auto sau.
- Quarantine trước xóa.
- Exact duplicate trong MVP; similar duplicate sau MVP.
- Project/FL Studio được bảo vệ, chưa parse dependency trong MVP.
- 14–16 tuần cho MVP public đáng tin cậy nếu làm full-time.

Các quyết định cần chủ sản phẩm duyệt trước Sprint 1:

1. Tên mã `MH FileOS` có giữ hay đổi.
2. MVP chỉ miễn phí hay chuẩn bị nền Free/Pro.
3. Quarantine mặc định nằm theo từng volume hay trong một vùng trung tâm.
4. Có cho quét toàn ổ C ở chế độ read-only ngay MVP hay chỉ cho chọn thư mục.
5. Mức ưu tiên của Inbox Suggest Mode: P1 cuối MVP hay bản 1.1.

Các quyết định này không chặn việc dựng scanner/catalog prototype, nhưng phải chốt trước khi thiết kế onboarding và release policy.

---

## PHỤ LỤC A — ACCEPTANCE CRITERIA CỐT LÕI

### AC-SCAN-001

Khi người dùng chọn một root có 100.000 file, scanner phải hoàn tất hoặc dừng theo lệnh mà không làm thay đổi mtime, nội dung hoặc vị trí của bất kỳ file nào.

### AC-DUP-001

Hai file chỉ được đánh dấu `duplicate_confirmed` khi size và full hash bằng nhau trên snapshot còn hiệu lực.

### AC-PLAN-001

Plan không được Execute nếu source size/mtime/file identity khác snapshot dùng để tạo plan.

### AC-MOVE-001

Move khác volume chỉ được committed sau khi destination được xác minh; source không bị retire trước bước verified.

### AC-UNDO-001

Undo chỉ trả success sau khi source path kỳ vọng tồn tại và đúng fingerprint; mọi lỗi phải giữ operation ở trạng thái recoverable/attention required.

### AC-PROTECT-001

Một path đi qua junction vào protected zone vẫn bị chặn sau canonicalization.

### AC-CONFLICT-001

Nếu destination đã tồn tại, hệ thống không được overwrite trong policy mặc định.

### AC-CRASH-001

Nếu app bị kill sau mỗi checkpoint của cross-volume move, lần mở tiếp theo phải xác định được Resume, Rollback hoặc Inspect mà không mất cả hai bản.

### AC-UI-001

Mọi con số dung lượng có thể thu hồi phải mở được danh sách file tạo nên con số đó.

### AC-I18N-001

Toàn bộ luồng onboarding, scan, plan, execution, recovery và error P0 phải có tiếng Việt hoàn chỉnh.

---

## PHỤ LỤC B — MẪU SAFETY INVARIANTS

1. Không operation nào được committed nếu trạng thái filesystem chưa được verify.
2. Không source nào bị loại bỏ trước khi destination hợp lệ được xác minh trong cross-volume move.
3. Không history nào được dùng làm nguồn sự thật thay cho operation journal.
4. Không plan nào được chạy nếu snapshot đã stale.
5. Không destination conflict nào được tự giải quyết bằng overwrite.
6. Không protected zone nào bị write bởi policy mặc định.
7. Không duplicate group nào được phép chọn loại bỏ toàn bộ bản.
8. Không shell command nào được tạo bằng nối chuỗi path.
9. Không AI output nào được chuyển thẳng thành destructive operation.
10. Không UI success nào được hiển thị khi core trả trạng thái không committed.

---

## PHỤ LỤC C — MẪU DEFINITION OF READY CHO SPRINT

Một task chỉ được đưa vào sprint khi có:

- User problem.
- Scope và non-scope.
- Acceptance criteria.
- Error cases.
- Data contract.
- Test approach.
- Security/safety impact.
- Dependency đã sẵn sàng.
- UI state nếu có.
- Ước lượng và owner.

---

**Kết luận:** MH FileOS nên được xây như một sản phẩm bảo vệ và tổ chức dữ liệu, không phải một script dọn file có giao diện đẹp. MVP thắng thị trường bằng độ tin cậy, khả năng giải thích và cảm giác kiểm soát; tự động hóa nâng cao chỉ là lớp phía trên của một lõi đã được chứng minh an toàn.
