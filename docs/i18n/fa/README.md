<div dir="rtl">

# RPoem — نمای کلی

- RPoem یک پلتفرم GraphQL Federation است که با Rust نوشته شده (سازگار با WunderGraph Cosmo).
- نقاط پایانی REST زیر /api/ در تاریخ 2026-11-01 به‌طور کامل حذف شدند. عملیات مدیریتی از طریق gRPC انجام می‌شود (همان تعریف به‌صورت gRPC-web و Connect نیز پذیرفته می‌شود) و روی یک درگاه دریافت می‌شود.
- GraphQL فقط برای پرس‌وجوهای Federation به کار می‌رود و در محیط عملیاتی چهار حفاظت فعال است (اجباری‌بودن پرس‌وجوهای ذخیره‌شده، محدودیت عمق و پیچیدگی، غیرفعال‌بودن introspection، رد درخواست‌های دسته‌ای).
- gRPC نیاز به TLS 1.3 دارد و در صورت نیاز mTLS. رابط مرورگر با WASM (382 کیلوبایت) ساخته شده و RPC Inspector و افزونهٔ ابزارهای توسعه‌دهنده (apps/devtools-extension) دارد.
- اجزای مشترک پروتکل‌ها open-runo-connect-core / server / client هستند (مخزن قدیمی gRPC-web-Connect در RPoem ادغام شد).

</div>

> فارسی · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の README.md (ja/en).
