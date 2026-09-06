# Hướng Dẫn Workflow 5harness (Tiếng Việt)

Tài liệu này giải thích chi tiết cách dùng **5harness** — một CLI toàn cục
giúp biến bất kỳ repo phần mềm nào thành workspace có cấu trúc cho con người
và coding agent (AI) cùng làm việc.

> **Khẩu hiệu:** App là thứ user chạm vào. Harness là thứ agent chạm vào.

---

## Mục Lục

1. [5harness Là Gì?](#1-5harness-là-gì)
2. [Cài Đặt & Khởi Tạo](#2-cài-đặt--khởi-tạo)
3. [Sơ Đồ Tư Duy (Mental Model)](#3-sơ-đồ-tư-duy)
4. [Feature Intake — Cổng Vào](#4-feature-intake)
5. [Story — Đơn Vị Công Việc](#5-story)
6. [Decision — Quyết Định Kiến Trúc](#6-decision)
7. [Backlog — Danh Sách Cải Tiến](#7-backlog)
8. [Trace — Nhật Ký Phiên](#8-trace)
9. [Công Cụ Đọc](#9-công-cụ-đọc)
10. [Vòng Lặp Chất Lượng](#10-chất-lượng)
11. [Quy Tắc Bắt Buộc](#11-quy-tắc)
12. [Workflow Từng Bước](#12-workflow)
13. [Cheatsheet CLI](#13-cheatsheet)

---

## 1. 5harness Là Gì?

**5harness** (package npm `5harness`, bin `harness`) là CLI toàn cục giúp:

1. **Khởi tạo** (`init`) project với agent docs + templates + conventions
2. **Liên kết** (`link`) project vào registry máy local (cho dashboard)
3. **Lưu trữ** intake/story/decision/backlog dạng **markdown** Git-backed
4. **Đánh index** để agent search/get thay vì đọc toàn bộ file
5. **Giữ trace** ở máy local (không làm nhiễu Git)

### Tổ Chức Thư Mục

```
project/
├── AGENTS.md                  ← Entrypoint cho agent
├── docs/
│   ├── HARNESS.md             ← Cách người & agent hợp tác
│   ├── FEATURE_INTAKE.md      ← Cổng intake & phân loại rủi ro
│   ├── ARCHITECTURE.md        ← Stack & kiến trúc
│   ├── CONTEXT_RULES.md       ← Quy tắc đọc context
│   ├── stories/               ← Story packets (*.md) — SoT
│   ├── decisions/             ← Decision records (*.md) — SoT
│   ├── intakes/               ← Intake records (*.md) — SoT
│   ├── backlog/               ← Backlog items (*.md) — SoT
│   └── product/               ← Product docs, roadmap
├── .5harness/
│   ├── index/                 ← Derived index (gitignored)
│   └── local/                 ← Traces (machine-local)
└── ~/.5harness/               ← Global registry
```

---

## 2. Cài Đặt & Khởi Tạo

```bash
# Cài toàn cục (khuyên dùng)
npm i -g 5harness
harness --version

# Khởi tạo project mới
cd /path/to/project
harness init

# Clone repo đã có harness
git clone <repo-url> && cd <repo>
harness link
```


## 3. Sơ Đồ Tư Duy

Mọi task đi qua pipeline có cấu trúc:

```
Ý định của Con Người
        │
        ▼
┌───────────────────┐
│  FEATURE INTAKE   │  ← Phân loại, chọn lane rủi ro
└───────────────────┘
        │
        ▼
┌───────────────────┐
│   STORY PACKET    │  ← Chia nhỏ công việc
└───────────────────┘
        │
        ▼
┌───────────────────┐
│  AGENT WORK LOOP  │  ← Agent thực hiện code/test
└───────────────────┘
        │
        ▼
┌───────────────────┐
│  PRODUCT DELTA    │  ← Code, test, docs thay đổi
└───────────────────┘
        │
        ▼
┌───────────────────┐
│ VALIDATION PROOF  │  ← Chứng minh hoàn thành
└───────────────────┘
        │
        ▼
┌───────────────────┐
│  HARNESS DELTA    │  ← Cập nhật harness
└───────────────────┘
        │
        ▼
┌───────────────────┐
│   NEXT INTENT     │  ← Ý định tiếp theo
└───────────────────┘
```

Mỗi task cho **2 output**:

| Output | Mô tả |
|--------|-------|
| **Product Delta** | Code, test, API, data model, docs thay đổi |
| **Harness Delta** | Docs, templates, backlog, decisions giúp task sau |

---

## 4. Feature Intake — Cổng Vào

**Intake** là bước đầu tiên: mọi yêu cầu được phân loại trước khi làm.

### Quy trình

```
User Prompt → Phân loại type → Diễn đạt work item
    → Tìm docs/story bị ảnh hưởng → Risk checklist → Chọn lane
```

### Input Types

| Type | Khi dùng | Đầu ra |
|------|---------|--------|
| **new_spec** | Biến spec user thành docs | Product docs, epics |
| **spec_slice** | Implement behavior đã chấp nhận | Story packet |
| **change_request** | Sửa/đổi behavior đã có | Story packet |
| **new_initiative** | Product area lớn, nhiều story | Initiative + stories |
| **maintenance_request** | Kỹ thuật, vận hành, dep | Story hoặc report |
| **harness_improvement** | Cải thiện harness | Docs update hoặc backlog |

### Risk Checklist (10 flags)

| Flag | Áp dụng khi chạm vào |
|------|---------------------|
| Auth | Login, session, JWT, password |
| Authorization | Roles, permissions, tenant scope |
| Data model | Schema, migration, deletion |
| Audit/security | Audit log, privacy, sensitive data |
| External systems | Email, payment, cloud, webhooks |
| Public contracts | API shape, client-visible behavior |
| Cross-platform | Desktop/mobile/browser |
| Existing behavior | Code/test đã có bị thay đổi |
| Weak proof | Thiếu test ở khu vực bị ảnh hưởng |
| Multi-domain | Nhiều domain thay đổi cùng lúc |

### Phân loại lane

| Số flags | Lane | Yêu cầu |
|----------|------|---------|
| 0–1 | **Tiny** | Intake → patch trực tiếp, giữ docs |
| 2–3 | **Normal** | Story packet, link docs, validation |
| 4+ | **High-Risk** | High-risk folder, confirm, decision |

**Hard gates** (luôn high-risk): Auth, Authorization, Data loss,
Audit/security, External providers, Xóa validation.

### CLI

```bash
harness intake --type spec_slice --summary "Thêm export API" --lane normal
harness intake --type change_request --summary "Sửa lỗi phân trang" --lane normal
```


## 5. Story — Đơn Vị Công Việc

**Story** là đơn vị công việc chính, mô tả cần làm gì và cách chứng minh xong.

### Cấu trúc Story Packet

```markdown
# US-XXX Story Title
## Status          ← planned | in_progress | implemented
## Lane            ← tiny | normal | high-risk
## Product Contract ← Behavior story phải làm đúng
## Relevant Docs   ← Link docs liên quan
## Acceptance Criteria ← Tiêu chí chấp nhận
## Design Notes    ← Commands, queries, API, tables
## Validation      ← Unit / Integration / E2E / Platform
## Harness Delta   ← Cập nhật harness từ story này
## Evidence        ← Command, report, screenshot
```

### Proof Flags (0 hoặc 1)

| Flag | Ý nghĩa |
|------|---------|
| **unit** | Unit test pass |
| **integration** | Integration test pass |
| **e2e** | End-to-end test pass |
| **platform** | Cross-platform smoke pass |

### CLI

```bash
# Tạo story
harness story add --id US-001 --title "Export API" --lane normal

# Cập nhật status + proof
harness story update --id US-001 --status implemented \
  --unit 1 --integration 1 --e2e 0 --platform 0

# Verify story
harness story verify --id US-001 --allow-project-command
harness story verify-all --allow-project-command
```

---

## 6. Decision — Quyết Định Kiến Trúc

**Decision** là bản ghi quyết định đã chốt, tránh tranh luận lặp lại.

### Khi nào cần

- Thay đổi architecture, authorization, data ownership
- Thay đổi API shape hoặc validation
- Quyết định auth, security, audit

### Cấu trúc: 2 phần

1. File markdown `docs/decisions/NNNN-*.md` (theo template)
2. Durable record qua CLI

### CLI

```bash
harness decision add \
  --id 0011 \
  --title "Dùng Markdown SoT thay SQLite" \
  --doc docs/decisions/0011-markdown-sot.md
```


## 7. Backlog — Danh Sách Cải Tiến

**Backlog** ghi nhận ý tưởng cải tiến, vấn đề, hoặc friction phát hiện khi làm.

### Khi nào thêm

- Friction lặp lại (quy trình, tooling, docs thiếu)
- Cần cải thiện harness nhưng chưa làm ngay
- Thiếu validation, docs lỗi thời

### Cấu trúc

| Trường | Ý nghĩa |
|--------|---------|
| **title** | Tiêu đề |
| **while** | Đang làm gì thì phát hiện |
| **pain** | Vấn đề gây khó khăn |
| **suggestion** | Đề xuất cải thiện |
| **risk** | Rủi ro nếu không sửa |
| **predicted** | Dự đoán kết quả |

### CLI

```bash
harness backlog add \
  --title "Thiếu test Windows" \
  --while "Chạy CI" \
  --pain "Không verify được cross-platform" \
  --suggestion "Thêm Windows smoke test"

harness backlog close --id BL-001 --outcome "Đã thêm, tất cả pass"
```

---

## 8. Trace — Nhật Ký Phiên

**Trace** ghi nhận phiên làm việc của agent: đọc gì, làm gì, kết quả, friction.

- Machine-local (`.5harness/local/`), không commit Git
- Dùng cho audit, handoff, cải thiện harness

### CLI

```bash
harness trace --story US-001 --summary "Implemented export" --outcome success
harness score-trace --id <trace-id>
```


## 9. Công Cụ Đọc

Agent **không** dump toàn bộ markdown tree. Dùng công cụ đọc có mục tiêu:

| Lệnh | Tác dụng |
|------|---------|
| `harness search "từ khóa"` | Tìm entity với ranked hits |
| `harness get <id>` | Đọc toàn bộ entity |
| `harness links <id>` | Outbound/backlinks/broken targets |
| `harness query matrix` | Bảng story (status + proof) |
| `harness query stats` | Thống kê số lượng |
| `harness query intakes` | Danh sách intake |
| `harness query decisions` | Danh sách decision |
| `harness query backlog` | Danh sách backlog |
| `harness query traces` | Danh sách trace |
| `harness query tools` | Tools đã đăng ký |

---

## 10. Vòng Lặp Chất Lượng

### Verify — Chạy proof

```bash
harness story verify --id US-001 --allow-project-command     # Chạy verify command trong story
harness story verify-all --allow-project-command             # Tất cả story có verify
```

### Audit — Kiểm tra drift

```bash
harness audit    # Entropy score 0–100, càng thấp càng tốt
```

### Propose — Đề xuất cải thiện

```bash
harness propose            # Đề xuất từ friction + audit
harness propose --commit   # Tạo backlog items
```

### Doctor — Kiểm tra sức khỏe

```bash
harness doctor             # Index, registry, entity dirs
harness reindex            # Build lại index (auto sau mutation)
```


## 11. Quy Tắc Bắt Buộc

### 🔴 Agent Mutation Rule

**Agent không tự tay sửa file markdown** story/decision/intake/backlog.
Mọi thay đổi qua CLI.

| ✅ Cho phép | ❌ Cấm |
|------------|-------|
| `harness intake ...` | Sửa tay file `docs/intakes/*.md` |
| `harness story add/update ...` | Sửa tay file `docs/stories/*.md` |
| `harness decision add ...` | Sửa tay file `docs/decisions/*.md` |
| `harness backlog add/close ...` | Sửa tay file `docs/backlog/*.md` |

> Tất cả mutation command đều **auto-reindex** sau khi ghi.

### 🔴 Hard-Fail Contract

Nếu CLI/MCP thất bại (non-zero exit):

| # | Hành động |
|---|-----------|
| 1 | **HARD STOP** — dừng durable-write path |
| 2 | **Không** fallback sửa tay entity |
| 3 | Phục hồi: `doctor` → `link` → `reindex` |
| 4 | Thử lại CLI ban đầu |

### 🟡 Cần xin confirm trước khi

- Thay đổi hướng kiến trúc
- Xóa validation requirements
- Thay đổi Source-of-Truth hierarchy
- Thay đổi risk classification
- Thay thế feature workflow

---

## 12. Workflow Từng Bước

### Flow chuẩn

```
harness intake ──→ harness story add ──→ Implement
       │                                      │
       └──────────────────────────────────────┘
                                              │
                                              ▼
                                    harness story update
                                              │
                                              ▼
                                       harness trace
                                              │
                              ┌───────────────┴───────────────┐
                              ▼                               ▼
                     harness backlog add             harness decision add
                              │                               │
                              └───────────────┬───────────────┘
                                              ▼
                                       harness audit
                                              │
                                              ▼
                                     harness propose
```

### Bước 1: Intake

```bash
harness intake --type spec_slice --summary "Thêm export CSV" --lane normal
```

### Bước 2: Story

```bash
harness story add --id US-099 --title "Export CSV" --lane normal
```

### Bước 3: Implement → Verify

```bash
harness story update --id US-099 --status implemented \
  --unit 1 --integration 1 --e2e 0 --platform 0 \
  --evidence "Unit: 12/12 pass. Integration: 5/5 pass."
```

### Bước 4: Trace

```bash
harness trace --story US-099 --summary "Done" --outcome success
```

### Bước 5: Harness Delta (nếu cần)

```bash
# Friction → backlog
harness backlog add --title "Thiếu template CSV test" ...

# Quyết định quan trọng → decision
harness decision add --id 0020 --title "Streaming CSV" ...
```

### Bước 6: Audit định kỳ

```bash
harness audit && harness query stats
```


## 13. Cheatsheet CLI

### Khởi tạo & Bảo trì
```bash
harness init                  # Khởi tạo project mới
harness link                  # Link clone có sẵn
harness unlink                # Hủy link
harness projects              # Liệt kê project
harness doctor                # Sức khỏe workspace
harness reindex               # Build lại index
harness upgrade               # Nâng cấp AGENTS.md block
harness status                # Snapshot project
harness next                  # Gợi ý việc tiếp theo
```

### Intake
```bash
harness intake --type <type> --summary "<text>" --lane <lane>
```

### Story
```bash
harness story add --id US-XXX --title "..." --lane <lane>
harness story update --id US-XXX --status implemented --unit 1 ...
harness story verify --id US-XXX --allow-project-command
harness story verify-all --allow-project-command
```

### Decision
```bash
harness decision add --id NNNN --title "..." --doc docs/decisions/...
harness decision verify --id NNNN --allow-project-command
```

### Backlog
```bash
harness backlog add --title "..." --while "..." --pain "..."
harness backlog close --id BL-XXX --outcome "..."
```

### Đọc & Tra cứu
```bash
harness search "<từ khóa>"      # Tìm entity
harness get <id>                 # Đọc entity
harness links <id>               # Links của entity
harness query matrix             # Bảng story
harness query stats              # Thống kê
harness query intakes|decisions|stories|backlog|traces
```

### Chất lượng
```bash
harness trace --story US-XXX --summary "..." --outcome success
harness score-trace <trace-id>
harness audit
harness propose [--commit]
```

### Khác
```bash
harness dashboard                # Mở dashboard
harness context <id>             # Context pack
harness handoff [--story US-XXX] # Session handoff
harness import-sqlite <path>     # Import SQLite cũ
harness export changelog         # Xuất changelog
```

---

## Định Nghĩa Done

Task được coi là **done** khi:

- ✅ Thay đổi đã hoàn thành (hoặc blocker đã document)
- ✅ Docs, stories, test matrix còn hiện hành
- ✅ Validation đã chạy (nếu có)
- ✅ Trace đã ghi với `harness trace`
- ✅ Missing harness đã ghi với `harness backlog add`
- ✅ Final response nói rõ thay đổi gì, chưa làm gì

---

> **Tham khảo:** `docs/HARNESS.md`, `docs/FEATURE_INTAKE.md`,
> `docs/ARCHITECTURE.md`, `docs/CONTEXT_RULES.md`, `docs/GLOSSARY.md`
