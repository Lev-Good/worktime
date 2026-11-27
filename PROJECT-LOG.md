# יומן הפרויקט — WorkTime

מסמך זה מתעד **את כל מה שנעשה** בפרויקט WorkTime: מה נבנה, איך, למה, מה נבדק בפועל, מה פורסם,
מה עדיין לא אומת, ואילו הצעות המשך קיימות. המסמך נכון ל־**27 בנובמבר 2026** (גרסה `1.0.0`).

> המסמך הוא תיעוד בלבד. סעיף [הצעות המשך](#הצעות-המשך--לא-מומשו) הוא רשימת הצעות **שלא מומשו**.

---

## תוכן עניינים

1. [תקציר](#תקציר)
2. [מה נבנה — לפי תחומים](#מה-נבנה--לפי-תחומים)
3. [ארכיטקטורה והחלטות טכניות](#ארכיטקטורה-והחלטות-טכניות)
4. [מבנה הקוד](#מבנה-הקוד)
5. [סכימת הנתונים](#סכימת-הנתונים)
6. [פקודות ה-backend](#פקודות-ה-backend)
7. [תפריט שורת המשימות](#תפריט-שורת-המשימות)
8. [השעון הצף](#השעון-הצף)
9. [תזכורות והתראות](#תזכורות-והתראות)
10. [גיבוי ושחזור](#גיבוי-ושחזור)
11. [בדיקת עדכוני גרסה](#בדיקת-עדכוני-גרסה)
12. [תהליך השחרור](#תהליך-השחרור)
13. [ה-workflow של GitHub Actions](#ה-workflow-של-github-actions)
14. [מה נבדק בפועל](#מה-נבדק-בפועל)
15. [היסטוריית גיט והמהדורה](#היסטוריית-גיט-והמהדורה)
16. [באגים שנתפסו ותוקנו](#באגים-שנתפסו-ותוקנו)
17. [מגבלות ידועות](#מגבלות-ידועות)
18. [הצעות המשך — לא מומשו](#הצעות-המשך--לא-מומשו)
19. [נספח: פקודות ומשתני סביבה](#נספח-פקודות-ומשתני-סביבה)

---

## תקציר

**WorkTime** היא תוכנת Windows קומפקטית לתיעוד שעות עבודה, ניהול לקוחות ופרויקטים, דוחות ותשלומים,
שמנוהלת כמעט כולה מתוך התפריט של סמל התוכנה בשורת המשימות, עם שעון צף מינימליסטי על המסך.

| | |
|---|---|
| סטאק | Tauri 2 + Rust + React 18 + TypeScript + Vite |
| מסד נתונים | SQLite מוטמע (`rusqlite` עם `bundled`) — ללא התקנה וללא שרת |
| ממשק | עברית מלאה, RTL, ללא ספריות UI וללא ספריות גרפים |
| מתקינים | NSIS (`setup.exe`, ~2.5MB) ו-MSI (~3.3MB) |
| מאגר | <https://github.com/Lev-Good/worktime> (ציבורי) |
| מהדורה | [`v1.0.0`](https://github.com/Lev-Good/worktime/releases/tag/v1.0.0) |
| רישיון | MIT |

המטרות שהנחו את הבנייה: קובץ התקנה קטן, צריכת זיכרון נמוכה, תחושת מוצר מלוטשת, אפס תלות בצד שרת
ואפס טלמטרייה.

---

## מה נבנה — לפי תחומים

### 1. עבודה ותיעוד זמן
- **טיימר חי** עם פרויקט, לקוח ותיאור; הזמן והשווי מתעדכנים בזמן אמת דרך `sync_active_entry`
  וטיקר ב-Rust (`commands::spawn_ticker`).
- **התחלה מהירה מהתפריט**: שמונת הפרויקטים האחרונים כפריטי תפריט, בתוספת "פרויקט חדש".
- **רשומות ידניות**: הוספה, עריכה ומחיקה, כולל שעת התחלה/סיום ותיאור.
- **סימון "לחיוב" ו"שולם"** לכל רשומה, כולל עדכון גורפת בעת רישום תשלום.
- **עיגול דקות** (5/10/15/30) לפי הגדרה.
- **סגירה אוטומטית** של טיימר שנשכח אחרי N שעות (`autoStopHours`, ברירת מחדל 12), עם התראה.

### 2. לקוחות, פרויקטים ופרופילים
- **לקוחות**: שם, איש קשר, אימייל, טלפון, כתובת, הערות, תעריף שעתי, ארכיון, סך שעות וחוב פתוח.
- **פרויקטים**: שיוך ללקוח, תיאור, סטטוס, תעריף, תקציב, צבע והתקדמות מול התקציב.
- **היררכיית תעריפים** (`db::resolve_rate`): תעריף הפרויקט → תעריף הלקוח → תעריף הפרופיל.
  התעריף שנקבע נשמר **על גבי הרשומה** (`rate`, `amount`), כך ששינוי תעריף בעתיד לא משנה היסטוריה.
- **מספר פרופילים נפרדים** (עבודה / פרילנס / עסק) עם הפרדת נתונים מלאה בכל השאילתות,
  ומעבר מהיר מהתפריט.

### 3. כספים ודוחות
- **דוח לפי טווח תאריכים** עם פילוח לפי יום, לקוח ופרויקט.
- סיכומים: סך שעות, שעות לחיוב, סכום לחיוב, שולם, ממתין לתשלום.
- **ייצוא CSV** עם BOM כדי שעברית תיפתח נכון באקסל.
- **רישום תשלומים** (סכום, מטבע, אמצעי, הערה, תאריך), כולל סגירה גורפת של שעות מחויבות
  וסיכום חובות פתוחים.

### 4. חוויית שימוש
- **ערכת נושא**: כהה / בהיר / לפי Windows.
- **שעון צף** שקוף, תמיד מעל החלונות, draggable, עם התחל/עצור, מצב קומפקטי, בקרת אטימות
  והצגת שניות.
- **התראות Windows ומרכז התראות פנימי**.
- **הפעלה אוטומטית עם Windows** (אופציונלי, עם `--hidden`).
- **מופע יחיד**: פתיחה שנייה מחזירה את החלון הקיים (`tauri-plugin-single-instance`).
- **סגירת חלון משאירה את התוכנה ברקע** (`CloseRequested` → `prevent_close` + `hide`).

### 5. נתונים, גיבוי ועדכונים
- **גיבוי מקומי ל-JSON** ולשחזור.
- **גיבוי ושחזור ל-Google Drive** בהרשאת `drive.file` בלבד, כולל גיבוי אוטומטי כל N שעות
  ושמירת 30 הגיבויים האחרונים.
- **בדיקת עדכוני גרסה** מול GitHub Releases, כולל הערות גרסה וקישורי הורדה.
- **תיעוד**: `README.md` מלא בעברית, `CHANGELOG.md` בפורמט Keep a Changelog, רישיון MIT.

---

## ארכיטקטורה והחלטות טכניות

| החלטה | הנימוק |
|---|---|
| **Tauri 2 ולא Electron** | מתקין וזיכרון קטנים בסדר גודל; WebView2 משותף עם Windows. |
| **Rust + SQLite (`rusqlite`, `bundled`)** | SQLite מהודר לתוך הבינארי — אין צורך בהתקנת מסד נתונים אצל המשתמש. |
| **ללא ספריות UI וללא ספריות גרפים** | שליטה מלאה בעיצוב, גודל קטן, אפס תלויות צד-שלישי בצד הלקוח. כל הרכיבים והתרשימים הם קוד של הפרויקט (`src/components/ui.tsx`, `Icon.tsx`). |
| **אין שרת ואין טלמטרייה** | פרטיות מלאה; קריאות הרשת היחידות הן GitHub (עדכונים) ו-Google Drive (גיבוי), ושתיהן ניתנות לכיבוי. |
| **שתי נקודות כניסה** (`index.html`, `floating.html`) | החלון הצף הוא webview נפרד וקל, לא טעינה חוזרת של האפליקציה המלאה. |
| **התפריט בשורת המשימות כממשק ראשי** | הדרישה המרכזית: ניהול מלא בלי לפתוח חלון. כל פעולה מרכזית קיימת בתפריט. |
| **תעריף נשמר לכל רשומה** | דוחות היסטוריים נשארים נכונים גם כשהתעריף משתנה. |
| **הערות הגרסה נגזרות מ-`CHANGELOG.md`** | מקור אמת אחד: מה שנכתב ב-CHANGELOG הוא בדיוק מה שמופיע במהדורה ובתוך התוכנה (`updates.rs` מציג את `body` של המהדורה). |
| **סקריפטי Node ללא תלויות** (`make-icons.mjs`, `release.mjs`) | אין `devDependencies` נוספים, ואותם סקריפטים רצים גם ב-CI וגם מקומית. |
| **CSP מהודק** | `default-src 'self'` בלבד, ללא `unsafe-eval`; ה-IPC מורשה במפורש. |
| **פרופיל release מכווץ** | `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`. |

---

## מבנה הקוד

```
src/                        ממשק React (עברית, RTL)
  App.tsx                   מסגרת האפליקציה, ניווט בין המסכים
  floating.tsx              חלון השעון הצף (נקודת כניסה נפרדת)
  components/ui.tsx         רכיבי UI משותפים (ללא ספריות)
  components/Icon.tsx       מערך אייקונים וקטורי בבעלות הפרויקט
  views/                    Dashboard, Entries, Clients, Projects, Reports,
                            Payments, Notifications, Profiles, Settings
  lib/api.ts                שכבת קריאה ל-Rust (invoke)
  lib/store.tsx             מצב גלובלי + האזנה לאירועי backend
  lib/format.ts             פורמט שעות/כסף/תאריכים
  lib/types.ts              טיפוסים משותפים
  styles.css                עיצוב, ערכות נושא, RTL
src-tauri/
  src/lib.rs                הרצת האפליקציה, plugins, חלון צף, רישום פקודות
  src/db.rs                 פתיחה, PRAGMA-ים, סכימה, ברירות מחדל, חישובי תעריף
  src/commands.rs           כל פקודות ה-backend (כ-1,340 שורות)
  src/models.rs             מבני נתונים ל-Serde
  src/tray.rs               תפריט שורת המשימות (כ-465 שורות)
  src/reminders.rs          בדיקת תזכורות רקע
  src/gdrive.rs             OAuth2 + PKCE + Drive API
  src/backup.rs             ייצוא/ייבוא JSON
  src/updates.rs            בדיקת מהדורות GitHub
  tauri.conf.json           חלון, CSP, באנדל NSIS/MSI
  Cargo.toml                תלויות ופרופיל release
scripts/
  make-icons.mjs            יצירת PNG/ICO ללא תלויות (כולל crc32/deflate ידניים)
  release.mjs               באמפ גרסה, קטע CHANGELOG ובדיקות שחרור
.github/workflows/release.yml   בנייה ופרסום מתקינים ב-Windows
```

בהיקף: כ-3,800 שורות Rust וכ-3,600 שורות TypeScript/TSX (לפני קבצי `lib/` ו-`styles.css`).

---

## סכימת הנתונים

המסד נפתח עם `PRAGMA journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`, `busy_timeout=4000`.

| טבלה | שדות עיקריים |
|---|---|
| `profiles` | `id`, `name`, `hourly_rate`, `currency`, `color`, `is_active`, `created_at` |
| `clients` | `id`, `profile_id`, `name`, `contact`, `email`, `phone`, `address`, `notes`, `hourly_rate`, `archived` |
| `projects` | `id`, `profile_id`, `client_id`, `name`, `description`, `status`, `hourly_rate`, `budget`, `color`, `archived` |
| `entries` | `id`, `profile_id`, `project_id`, `client_id`, `description`, `started_at`, `ended_at`, `duration_sec`, `rate`, `amount`, `billable`, `paid` |
| `payments` | `id`, `profile_id`, `client_id`, `project_id`, `amount`, `currency`, `method`, `note`, `paid_at` |
| `notifications` | `id`, `title`, `body`, `kind`, `created_at`, `read` |
| `settings` | `key`, `value` (מפתח-ערך) |

אינדקסים: `entries(started_at)`, `entries(profile_id, started_at)`, `entries(project_id)`,
`payments(paid_at)`.

טיימר פעיל הוא שורה ב-`entries` עם `ended_at IS NULL`; `sync_active_entry` מרענן את
`duration_sec`/`amount` שלה בכל טיק, ולכן כל שאילתת אגרגציה נשארת `SUM` פשוט על עמודות שמורות.

### מפתחות הגדרות (ברירות מחדל)

`theme=dark`, `currency=ILS`, `workdayHours=8.5`, `dailyGoalHours=8`, `workStart=09:00`,
`workEnd=18:00`, `idleReminderMinutes=45`, `notificationsEnabled=true`, `roundingMinutes=0`,
`autoStopHours=12`, `floatingEnabled=false`, `floatingSeconds=true`, `floatingOpacity=0.94`,
`floatingAlwaysOnTop=true`, `floatingCompact=false`, `trayShowTimer=true`, `autostart=false`,
`autoCheckUpdates=true`, `updateRepo=Lev-Good/worktime`, `backupEnabled=false`,
`backupIntervalHours=24`, `gdriveClientId`, `gdriveClientSecret`, `gdriveFolderId`,
`gdriveAccount`, `lastBackupAt`, `lastUpdateCheck`, `billableDefault=true`, `firstRun=true`.

---

## פקודות ה-backend

**ליבה**: `bootstrap`, `dashboard`, `timer`, `show_main`, `quit_app`, `notify`, `open_url`.

**לקוחות ופרויקטים**: `list_clients`, `save_client`, `delete_client`,
`list_projects`, `save_project`, `delete_project`.

**רשומות וטיימר**: `list_entries`, `start_timer`, `stop_timer`, `save_entry`, `delete_entry`,
`set_entry_paid`.

**כספים**: `report`, `export_report_csv`, `list_payments`, `save_payment`, `delete_payment`.

**הגדרות ופרופילים**: `save_settings`, `save_profile`, `delete_profile`, `switch_profile`,
`set_floating`.

**התראות ועדכונים**: `list_notifications`, `mark_notification`, `clear_notifications`,
`check_updates`, `backup_local`.

**Google Drive**: `gdrive_status`, `gdrive_connect`, `gdrive_disconnect`, `gdrive_backup`,
`gdrive_backups`, `gdrive_restore`.

---

## תפריט שורת המשימות

התפריט נבנה מחדש בכל שינוי מצב (`tray::build`) ומציג:

```
⏱ 01:23:45 · אתר תדמית        (פריט סטטוס, מושבת — טקסט חי)
▶ התחל עבודה / ⏹ עצור וסכם      (toggle)
פתח את WorkTime                (open)
התחל עבודה בפרויקט  ▸          (8 פרויקטים אחרונים + "פרויקט חדש")
פרופיל פעיל  ▸                 (CheckMenuItem לכל פרופיל)
שעון צף על המסך  ✓             (CheckMenuItem)
התראות  ✓                      (CheckMenuItem — משתיק)
היום: 3:24 שעות · ₪340         (פריט מידע)
ממתין לתשלום: ₪1,200           (פריט מידע)
לוח בקרה / יומן עבודה / לקוחות / פרויקטים
דוחות וסיכומים / תשלומים / התראות והודעות
גיבוי ל-Google Drive / בדוק עדכוני גרסה / הגדרות / יציאה
```

ניווט מהתפריט פותח את החלון הראשי ישירות במסך המבוקש (`nav:*` → `tray::open_main`).

---

## השעון הצף

- חלון `floating` נבנה דינמית: 252×92, שקוף, ללא מסגרת, `always_on_top`, `skip_taskbar`,
  `focused=false`, ממוקם בפינה השמאלית־תחתונה של המסך הראשי בהתחשב ב-scale factor.
- מציג זמן רץ, שם פרויקט וכפתור התחל/עצור; תומך במצב קומפקטי ובאטימות מוגדרת.
- מצב החלון נשמר בהגדרות (`floatingEnabled`) ונפתח אוטומטית בהפעלה.
- אם קיים כבר חלון — הוא מוצג מחדש ולא נוצר כפול.

---

## תזכורות והתראות

`reminders::check` רץ ברקע (דרך הטיקר) ובודק, רק אם `notificationsEnabled`:

1. **"הטיימר כבוי"** — בתוך שעות העבודה, כשאין טיימר פעיל ועברו `idleReminderMinutes`
   מאז הרשומה האחרונה. *ההגדרה מתייחסת לרשומות, לא לפעילות משתמש במערכת.*
2. **"הגעתם ליעד היומי"** — כשסך השעות היום עבר `dailyGoalHours`, עם הגבלת תדירות של 12 שעות.

התראה נשמרת גם ב-`notifications` (מרכז ההתראות) וגם כ-הודעת Windows, ונשלח אירוע
`state-changed` לרענון הממשק. תזכורות רקע אחרות: גרסה חדשה (אחרי בדיקת עדכונים) וגיבוי.

---

## גיבוי ושחזור

### מקומי (JSON)
`backup.rs` מייצא את כל הטבלאות ל-JSON אחד ושומר לקובץ שנבחר; הייבוא רץ בטרנזקציה,
משחזר הכל, ומסמן פרופיל פעיל אחד. קובץ שלא מכיל פרופילים נדחה.

### Google Drive
- **זרימת OAuth ידנית עם PKCE (S256)** ושרת loopback מקומי, כדי לא להסתמך על דפדפן מוטמע
  או על client secret בצד הלקוח. ה-`Client ID`/`Client Secret` מוזנים על ידי המשתמש
  בהגדרות ונשמרים מקומית.
- **הרשאה**: `https://www.googleapis.com/auth/drive.file` בלבד (גישה רק לקבצים שהתוכנה יצרה).
- **תיקייה**: `WorkTime Backups` בדרייב; **נשמרים 30 הגיבויים האחרונים** (`KEEP_BACKUPS`).
- **טוקן רענון** נשמר בקובץ `gdrive-token.json` בתיקיית הנתונים של האפליקציה (הרשאות 0600
  בפלטפורמות Unix), עם רענון אוטומטי לפני פקיעה.
- **שחזור** מחליף את הנתונים הנוכחיים; מפתחות סוד (`SECRET_KEYS`) מדולגים בייבוא גיבוי.
- גיבוי אוטומטי כל `backupIntervalHours` שעות, אם הופעל.

---

## בדיקת עדכוני גרסה

`updates::check(repo, current)` קורא `GET https://api.github.com/repos/{repo}/releases/latest`
(עם `User-Agent: WorkTime/<version>`, timeout 20 שניות) ומחזיר:

- `has_update` — השוואת semver (`v` prefix, `-prerelease`, `+build` נתמכים),
- `title`, `notes` (ה-`body` של המהדורה), `published_at`, `page_url`,
- `assets` — רק `.exe` / `.msi` / `.msi.zip`, עם שם, גודל וקישור הורדה ישיר.

ברירת המחדל: בדיקה אוטומטית פעם ב-12 שעות ברקע (נשמר ב-`lastUpdateCheck`), והתראה כשקיימת גרסה
חדשה. ה-API נקרא אנונימית — ולכן המאגר פורסם כציבורי, בלי טוקן מוטמע בתוכנה.

---

## תהליך השחרור

`scripts/release.mjs` (Node, ללא תלויות) — כלי יחיד לשחרור גרסה.

### `npm run release:prepare -- 1.0.1`
1. מאמת semver ומאמת שהגרסה **גדולה** מהנוכחית.
2. מעדכן את מספר הגרסה ב־4 מקומות: `package.json`, `src-tauri/tauri.conf.json`,
   `src-tauri/Cargo.toml`, ו-`src-tauri/Cargo.lock` (הסטנזה `name = "worktime"`).
3. מוסיף ל-`CHANGELOG.md` קטע `## [1.0.1] — YYYY-MM-DD` עם שורת `TODO` (אם אינו קיים),
   ושומר על סוג סוף השורה של הקובץ (LF/CRLF).
4. מריץ `npm run build` (typecheck + build של ה-frontend, בדיוק כמו ב-CI) — ניתן לדלג עם
   `--no-verify`, ולהריץ יבש עם `--dry-run`.
5. מדפיס את שלבי ההמשך (מילוי CHANGELOG, commit, tag, push).

### `npm run release:check`
מאמת: ארבעת הקבצים נושאים גרסה זהה ולא ריקה, קטע ה-CHANGELOG לאותו מספר גרסה קיים, אינו ריק
ואינו מכיל `TODO`, ושם התג (`GITHUB_REF_NAME`) תואם ל-`v<version>` כשקיים. בכל כשל — פלט מפורט
שאומר בדיוק איזה קובץ נשאר מאחור. הבדיקה רצה גם בתוך ה-workflow **לפני** הבנייה, כך שגרסה
לא עקבית לא תפורסם כתג.

### שלבי השחרור המלאים
```bash
npm run release:prepare -- 1.0.1
# עורכים את CHANGELOG.md במקום שורת ה-TODO
npm run release:check
git add -A && git commit -m "Release v1.0.1"
git tag v1.0.1
git push origin main v1.0.1
```

---

## ה-workflow של GitHub Actions

`.github/workflows/release.yml` — `Release Windows`, רץ על push של תג `v*` ועל `workflow_dispatch`, עם
`permissions: contents: write`, על `windows-latest`:

1. **Checkout** (`actions/checkout@v4`)
2. **Setup Node 20** עם cache ל-npm
3. **Setup Rust stable** (`dtolnay/rust-toolchain@stable`)
4. **Cache Rust** (`swatinem/rust-cache@v2`, workspace `./src-tauri -> target`)
5. **`npm ci`**
6. **`npm run release:check`** — שער איכות למטא-דאטה של הגרסה
7. **`npm run build`** — typecheck + בניית ה-frontend
8. **חילוץ הערות הגרסה מ-CHANGELOG** (PowerShell): קורא את הגרסה מ-`tauri.conf.json`,
   מוצא את הקטע `## [x.y.z]` עד הקטע הבא, ומוסיף כותרת הורדה/התקנה; נכתב ל-`GITHUB_OUTPUT`
   עם דלימיטר `__WORKTIME_NOTES__`.
9. **`tauri-apps/tauri-action@v0`** — בונה את שני המתקינים ומפרסם מהדורה עם
   `tagName: v__VERSION__`, `releaseName: WorkTime v__VERSION__`, `releaseDraft: false`,
   `prerelease: false`, ו-`releaseBody` מהשלב הקודם.

בנוסף: `.gitattributes` מקבע `eol=lf` לכל קבצי הטקסט ומסמן `*.ico/png/exe/msi` כבינאריים,
כדי שלא ייווצרו דיפים מדומים ולא יידחפו קבצים פגומים.

---

## מה נבדק בפועל

### בדיקות מקומיות (סביבת Linux, ללא Windows)
| בדיקה | תוצאה |
|---|---|
| `npx tsc --noEmit` | עבר |
| `npm run build` (tsc + vite build, שני ה-entry points) | עבר |
| `cargo check` ל-target של Linux (עם stub `pkg-config` עבור WebKitGTK) | עבר |
| `cargo check --target x86_64-pc-windows-msvc` | **לא ניתן** — אין MSVC/`lib.exe` בסביבה |
| `node scripts/release.mjs 1.0.1 --dry-run` | עבר, כולל אימות מיקומי ההחלפה |
| `node scripts/release.mjs --check` על הריפו | עבר |
| `--check` על עותק מבודד עם CRLF | עבר (אחרי התיקון המתואר למטה) |

### בדיקות קצה-לקצה של השחרור
- ריצת CI מלאה ראשונה: בנייה + פרסום המהדורה — **הצליחה** (run `#45930131356`).
- אימות אנונימי של ה-API: `GET /repos/Lev-Good/worktime/releases/latest` מחזיר `HTTP 200`
  עם התג, ההערות והנכסים — כלומר בדיקת העדכונים שבתוכנה תעבוד בלי טוקן.
- שלב `Verify release metadata` אומת בנפרד על `windows-latest` (success).
- שלב חילוץ ההערות אומת בריצה ידנית מלאה (`workflow_dispatch`).
- תוכן ההערות שפורסמו = קטע ה-CHANGELOG (18 שורות "נוסף") + הוראות התקנה; אורך ה-body
  שפורסם כ-1,399 תווים.

### מה **לא** אומת
- **לא נבדק ויזואלית** שום מסך של ה-GUI, לא ב-Windows ולא דרך דפדפן — אין Windows בסביבה.
  לא נבדקו גם התפריט בפועל, השעון הצף, drag, אטימות והתראות Windows.
- **לא הותקנו** המתקינים בפועל ולא נבדקה התקנה/הסרה/הרשאות משתמש.
- **זרימת OAuth מול Google** לא הורצה מקצה לקצה (דורשת פרויקט Google Cloud ואישור בדפדפן).
- **גיבוי/שחזור ל-Drive** לא נבדקו מול חשבון אמיתי.
- **בדיקת עדכון בתוך התוכנה** לא הורצה בתוך אפליקציה רצה; רק ה-API אומת מבחוץ.
- אין בדיקות יחידה או בדיקות אינטגרציה בקוד (ראו הצעה 8).

---

## היסטוריית גיט והמהדורה

| קומיט | תיאור |
|---|---|
| `18705af` | Initial cloud project |
| `d2a3e3f` | Import cloud project files |
| `fd0c551` | Add WorkTime — compact Windows work-time tracker |
| `2f2a550` | Polish settings about card and release notes template |
| `51665a2` | Fix topbar background fallback for older WebView2 and add nowrap utility |
| `bea00ee` | Ignore local source archives — **מסומן בתג `v1.0.0`** |
| `a9e8679` | Publish real release notes from CHANGELOG in the workflow |
| `92f4dd5` | Add a release script for bumping versions and verifying metadata |
| `94c99fc` | Make release checks line-ending agnostic and pin LF checkouts |

**מהדורה `v1.0.0`** (פורסמה ב-27.11.2026, לא draft ולא prerelease):

| קובץ | גודל |
|---|---|
| `WorkTime_1.0.0_x64-setup.exe` (NSIS) | 2,639,131 בייט (~2.5MB) |
| `WorkTime_1.0.0_x64_en-US.msi` | 3,473,408 בייט (~3.3MB) |

ההתקנה היא למשתמש הנוכחי (`installMode: currentUser`), ללא הרשאות מנהל, עם ערכות שפה
עברית ואנגלית.

---

## באגים שנתפסו ותוקנו

1. **רקע ה-topbar לא הוצג ב-WebView2 ישן** — נוסף צבע fallback במקום `color-mix`
   (`51665a2`), יחד עם `nowrap` לשורות סטטוס.
2. **הערות הגרסה היו גנריות** — ה-workflow פרסם טקסט קבוע; הוחלף בחילוץ אמיתי מקטע
   ה-CHANGELOG (`a9e8679`).
3. **הערות הגרסה כללו את כל הכותרת של הקובץ** — החילוץ עוצר בקטע `## [` הבא ומוסיף
   הוראות התקנה במקום.
4. **כשל CI על `windows-latest`: "Cargo.lock: לא נמצא מספר גרסה"** — קבצים נבדקו בסביבת
   Windows עם CRLF והרג'קס הניח LF. תוקן ב-`92f4dd5`/`94c99fc`: `\r?\n` בכל התבניות,
   `eolOf()` לשימור סוג סוף השורה, ו-`.gitattributes` שמקבע `eol=lf` בצ'קאאוט.
5. **ארכיון מקור מקומי נכנס לגיט** (`WorkTime-source.zip`) — נוסף ל-`.gitignore` (`bea00ee`).

---

## מגבלות ידועות

- **Windows בלבד.** אין build ל-macOS/Linux ולא נבדקה התאמה שלהן.
- **אין חתימת קוד (Authenticode)** — ייתכן אזהרת SmartScreen בהתקנה ראשונה.
- **עדכון הוא ידני**: התוכנה מציגה גרסה חדשה וקישור, אבל ההורדה וההתקנה נעשות על ידי המשתמש.
- **אין סנכרון בין מחשבים** — רק גיבוי/שחזור נקודתי דרך Drive.
- **תזכורת "הטיימר כבוי"** מבוססת רשומות ולא פעילות מערכת, וטיימר ממשיך לצבור זמן בזמן
  שינה/כיבוי מסך (עד לתקרת `autoStopHours`).
- **מחיקות הן סופיות** (לקוח/רשומה/תשלום) — אין סל אשפה או "בטל".
- **אין בדיקות אוטומטיות** בפרויקט.
- **OAuth ל-Drive דורש הגדרה עצמית** של פרויקט Google Cloud על ידי המשתמש (מתועד ב-README).

---

## הצעות המשך — לא מומשו

> כל הסעיפים כאן הם **הצעות בלבד**. שום דבר מהם לא נכתב בקוד. הדירוג לפי ערך למוצר.

| # | הרעיון | ערך | מאמץ |
|---|---|---|---|
| 1 | זיהוי חוסר פעילות אמיתי + טיפול בשינה/כיבוי | גבוה מאוד | בינוני |
| 2 | קיצור מקלדת גלובלי להתחלה/עצירה | גבוה מאוד | קטן |
| 3 | מחיקה עם שחזור (סל אשפה ל-30 יום) | גבוה | קטן-בינוני |
| 4 | דוח/חשבונית PDF ללקוח | גבוה | בינוני |
| 5 | עדכון אוטומטי בתוך התוכנית | גבוה | בינוני |
| 6 | ייבוא היסטוריה מ-Toggl/Clockify/CSV | בינוני-גבוה | בינוני |
| 7 | חיפוש חופשי + תגיות ביומן | בינוני | קטן |
| 8 | בדיקות אוטומטיות ללוגיקת החיוב | בינוני (מונע נזק עתידי) | קטן-בינוני |

### 1. זיהוי חוסר פעילות אמיתי וטיפול בשינה
כיום קיימת רק תזכורת "הטיימר כבוי" שמבוססת על הזמן מאז הרשומה האחרונה (`reminders.rs`),
ולא על פעילות אמיתית של המשתמש. טיימר שרץ ממשיך לצבור זמן בזמן שהמחשב ישן, והתקרה היחידה
היא `autoStopHours` (12 שעות, `db::auto_close_stale`) — כלומר שינה של שעתיים נכנסת לרישום,
ואם הפרויקט לחיוב, זה כסף שגוי מול הלקוח. שווה לחבר `GetLastInputInfo` (עם `windows` crate)
לבדיקה תקופתית, לסמן או לקצר זמן חוסר פעילות מעל סף, ולטפל באירועי sleep/resume.

### 2. קיצור מקלדת גלובלי להתחלה/עצירה
התוכנה מוגדרת כניהול מתוך שורת המשימות, אבל כל עצירה דורשת שני קליקים. קיצור גלובלי
(למשל `Ctrl+Alt+Space`) עם התראה קצרה מסיר את החיכוך בכל החלפת משימה — וזה המנוע של רישום אמין.
מאמץ קטן יחסית (`tauri-plugin-global-shortcut`), השפעה יומיומית מיידית.

### 3. מחיקה עם שחזור
`delete_entry` / `delete_client` / `delete_payment` מוחקים לצמיתות, והגיבוי האחרון הוא ידני
או עד 24 שעות אחורה. מחיקה בשוגג של שעות ששולמו אינה ניתנת לשחזור מתוך התוכנה. soft-delete
עם "בטל" בהתראה, או סל ל-30 יום, הוא הביטוח הזול ביותר שאפשר להוסיף.

### 4. דוח/חשבונית PDF ללקוח
קיים CSV — כלי ביניים, לא משהו שנשלח ללקוח. דוח תקופתי מעוצב (שעות לפי פרויקט, תעריף, סה"כ,
תקופה, פרטי העסק, אופציונלית מע"מ) עם "הדפס/שמור PDF" דרך HTML + `window.print`, בלי ספריית PDF,
סוגר את הלופ עד בקשת התשלום.
הערה: חשבונית מס ישראלית עם מספר הקצאה היא נושא נפרד ודורש אינטגרציה חיצונית.

### 5. עדכון אוטומטי בתוך התוכנית
`updates.rs` מציג גרסה חדשה, הערות וקישור — והמשתמש מוריד ומתקין ידנית. `tauri-plugin-updater`
עם חתימה יהפוך זאת ל"עדכן והפעל מחדש" בלחיצה, ויהפוך את עבודת ה-CHANGELOG והשחרור לבעלת ערך
מיידי. דורש מפתח חתימה ב-Secrets וב-workflow.

### 6. ייבוא היסטוריה
משתמש שמגיע מ-Toggl/Clockify/אקסל לא יזין שנתיים אחורה ידנית, ולכן הדוחות והתשלומים שלו
"מתחילים מאפס". ייבוא CSV ממופה עם תצוגה מקדימה מכניס את ההיסטוריה פנימה ומקטין את מחיר המעבר.

### 7. חיפוש חופשי ותגיות ביומן
הסינון הקיים הוא לפי טווח/פרויקט/לקוח; חיפוש טקסט בתיאור הוא הדבר הראשון שמבקשים כשמצטברות
מאות רשומות. תגיות/סוג עבודה (פיתוח / פגישות / אדמיניסטרציה) פותחות תובנה אמיתית: כמה מהשבוע
בכלל לחיוב. שניהם קטנים על הסכימה הקיימת.

### 8. בדיקות אוטומטיות ללוגיקת החיוב
אין כיום אף `#[test]` בפרויקט. החישובים הרגישים — `resolve_rate` (פרויקט→לקוח→פרופיל),
עיגול דקות, `round_amount`, סכומי דוח, סגירה גורפת כששולם — הם בדיוק המקום שבו שינוי "תמים"
שובר כסף בשקט. מספר בדיקות יחידה ושלב `cargo test` ב-CI מגדילים את הביטחון בכל שינוי עתידי.

### זריזים ששווה לשקול בהמשך (ערך קטן, מאמץ קטן)
- **"המשך את הרשומה האחרונה"** בלחיצה בתפריט/בלוח הבקרה — המדווח החוזר הנפוץ ביותר.
- **יעד הכנסה חודשי** לצד יעד השעות: "נשארו ₪2,400 ליעד".
- **גיבוי מקומי מסתובב** (7 קבצים בתיקיית הנתונים) לפני כל מיגרציה.
- **גיבוי מוצפן בסיסמה** כשהקובץ עולה לדרייב.

### שלא הומלץ לקדם עכשיו
- **סנכרון בין מחשבים דרך Drive** — מיזוג עם קונפליקטים, סיכון גבוה מול תועלת בינונית
  למי שעובד ממחשב אחד.
- **לוקליזציה לאנגלית** — הקהל המוצהר עברי; הממשק כבר RTL ועברית מלאה.
- **מעקב הוצאות** — מרחיב את המוצר מעבר לליבת ניהול הזמן.

---

## נספח: פקודות ומשתני סביבה

### פיתוח ובנייה
```bash
npm install
npm run tauri dev        # פיתוח
npm run build            # typecheck + build frontend
npm run tauri build      # מתקינים תחת src-tauri/target/release/bundle/
npm run icons            # יצירת אייקונים מחדש (ללא תלויות)
npm run release:prepare -- 1.0.1
npm run release:check
```

### נתיבי נתונים
- מסד: `%APPDATA%\com.levgood.worktime\worktime.db` (עם `-wal` / `-shm`).
- טוקן Drive: `%APPDATA%\com.levgood.worktime\gdrive-token.json`.

### פרסום (תיעוד תפעולי)
הדחיפה בוצעה באמצעות `GITHUB_TOKEN` שהיה קיים במשתני הסביבה, כ-credential helper זמני
בשורת הפקודה — **הטוקן לא נכתב לשום קובץ, לא הודפס ולא נשמר בריפו**. אין סודות בקוד או
בהיסטוריית הגיט.

```bash
git -c credential.helper='!f() { echo username=x-access-token; echo "password=${GITHUB_TOKEN}"; }; f' \
  push origin HEAD:main
```

### משתני סביבה רלוונטיים
| משתנה | שימוש |
|---|---|
| `GITHUB_TOKEN` | דחיפה למאגר (לא נדרש לבנייה) |
| `GITHUB_REF_NAME` | נקרא על ידי `release:check` בתוך CI כדי לאמת את שם התג |
| `TAURI_DEV_HOST` | פיתוח מרחוק (HMR) ב-Vite |
