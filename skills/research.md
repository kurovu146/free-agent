# Research & Web Investigation Skill

## Khi nào kích hoạt
Khi user yêu cầu: nghiên cứu, tìm hiểu, so sánh, research, investigate, "tìm hiểu về X", "X là gì", "so sánh A vs B", hoặc bất kỳ câu hỏi nào cần thông tin từ internet.

## Quy trình nghiên cứu (BẮT BUỘC tuân thủ)

### Bước 1: Search rộng
- Gọi `web_search` với 2-3 queries khác nhau để có nhiều góc nhìn
- Ví dụ: nếu user hỏi "tìm hiểu về Bun runtime", search cả:
  - "Bun runtime overview features"
  - "Bun vs Node.js comparison 2025"

### Bước 2: Fetch chi tiết
- Sau khi có kết quả search, PHẢI gọi `web_fetch` để đọc ít nhất 2-3 trang
- Ưu tiên: docs chính thức, blog kỹ thuật uy tín, GitHub repos
- KHÔNG chỉ dựa vào snippets từ search results — snippets thường thiếu context

### Bước 3: Tổng hợp & Cross-reference
- So sánh thông tin từ nhiều nguồn
- Nếu có mâu thuẫn: nói rõ "Nguồn A nói X, nguồn B nói Y"
- Ưu tiên thông tin mới nhất (check ngày xuất bản)

### Bước 4: Trình bày kết quả
Format bắt buộc:

```
## [Chủ đề]

**Tóm tắt**: 2-3 câu overview

### Key Points
- Point 1
- Point 2
- Point 3

### [Chi tiết theo yêu cầu — so sánh, ưu/nhược, cách dùng...]

### Nguồn tham khảo
- [Tên nguồn 1](URL1)
- [Tên nguồn 2](URL2)
- [Tên nguồn 3](URL3)
```

## Quy tắc quan trọng

1. **LUÔN trích nguồn** — Mỗi câu trả lời research PHẢI có section "Nguồn tham khảo" với URLs
2. **LUÔN dùng web_fetch** — KHÔNG chỉ dùng mỗi web_search rồi tóm tắt snippets
3. **Tự mở rộng** — Nếu kết quả search đầu tiên chưa đủ, tự search thêm với keywords khác
4. **Xác minh** — Nếu thông tin quan trọng, cross-check từ ít nhất 2 nguồn
5. **Đánh giá độ tin cậy** — Docs chính thức > Blog tech > Forum > Random website
6. **Ghi nhận giới hạn** — Nếu không tìm được đủ info, nói rõ thay vì bịa

## Ví dụ flow tool calls

User: "Tìm hiểu về Hono framework"

1. `web_search("Hono framework features overview")`
2. `web_search("Hono vs Express comparison")`
3. `web_fetch("https://hono.dev/docs/")` (docs chính thức)
4. `web_fetch("https://blog.example.com/hono-review")` (review bài viết)
5. Tổng hợp → trả lời với format trên + trích nguồn

## So sánh / Comparison
Khi user yêu cầu so sánh A vs B:

| Criteria | A | B |
|----------|---|---|
| Performance | ... | ... |
| DX | ... | ... |
| Ecosystem | ... | ... |
| Use case | ... | ... |

**Đề xuất**: Chọn 1, giải thích tại sao, kèm confidence level.
