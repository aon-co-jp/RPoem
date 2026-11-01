<div dir="rtl">

# RPoem — نظرة عامة

- RPoem منصة GraphQL Federation مكتوبة بلغة Rust (متوافقة مع WunderGraph Cosmo).
- أُلغيت نقاط REST الموجودة تحت /api/ بالكامل في 2026-11-01. تتم العمليات الإدارية عبر gRPC (ويُقبل التعريف نفسه أيضًا بصيغتي gRPC-web وConnect) على منفذ واحد.
- يُستخدم GraphQL لاستعلامات Federation فقط، وتُفعَّل في بيئة الإنتاج أربع حمايات (إلزام الاستعلامات المحفوظة، حدود العمق والتعقيد، تعطيل الاستبطان، رفض الطلبات الدفعية).
- يتطلب gRPC بروتوكول TLS 1.3 مع mTLS اختياريًا. واجهة المتصفح مبنية بـ WASM (382 كيلوبايت) وتضم RPC Inspector وإضافة لأدوات المطوّر (apps/devtools-extension).
- المكوّنات المشتركة للبروتوكولات هي open-runo-connect-core / server / client (دُمج المستودع القديم gRPC-web-Connect في RPoem).

</div>

> العربية · 2026-11-01 時点の要約 / summary of the state on 2026-11-01. 正本・詳細: リポジトリ直下の README.md (ja/en).
