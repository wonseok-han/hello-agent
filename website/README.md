# Hello, Agent website

Hello, Agent 데스크톱 앱을 소개하는 한국어 랜딩 페이지입니다. 앱과 분리된
배포 단위이며, OpenAI Sites와 Cloudflare Worker 호환 빌드를 사용합니다.

```bash
npm install
npm run dev
npm test
```

- 제품 페이지: `app/page.tsx`
- 전역 스타일: `app/globals.css`
- 메타데이터: `app/layout.tsx`
- Sites 설정: `.openai/hosting.json`
- 브랜드 아이콘 원본: `../brand/hello-agent-mark.svg`
