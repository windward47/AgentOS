#!/usr/bin/env node
import { chromium } from 'playwright';

const url = process.argv[2];
const outPath = process.argv[3];
const timeout = parseInt(process.argv[4] || '15000');

if (!url || !outPath) {
  console.error('Usage: node browser-screenshot.mjs <url> <output.png> [timeout_ms]');
  process.exit(1);
}

try {
  const browser = await chromium.launch({ channel: 'chrome', headless: true });
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  await page.goto(url, { waitUntil: 'networkidle', timeout });
  await page.screenshot({ path: outPath, fullPage: false });
  await browser.close();
  console.log(outPath);
} catch (e) {
  console.error('BROWSER_ERROR:', e.message);
  process.exit(1);
}
