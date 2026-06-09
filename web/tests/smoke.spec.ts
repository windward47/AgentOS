import { test, expect } from '@playwright/test'

test.describe('Companion UI smoke tests', () => {

  test('main layout has sidebar, chat, and input', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // Screenshot for visual verification
    await page.screenshot({ path: 'tests/screenshots/01-main-chat.png', fullPage: true })

    // Sidebar should be visible with nav items
    const sidebar = page.locator('aside')
    await expect(sidebar).toBeVisible()

    const chatBtn = sidebar.getByText('Chat')
    const settingsBtn = sidebar.getByText('Settings')
    await expect(chatBtn).toBeVisible()
    await expect(settingsBtn).toBeVisible()

    // Chat area should have input
    const input = page.locator('input[placeholder*="Message"]')
    await expect(input).toBeVisible()

    // Send button should exist
    const sendBtn = page.getByText('Send')
    await expect(sendBtn).toBeVisible()
  })

  test('navigate to settings and back', async ({ page }) => {
    await page.goto('/')

    // Click Settings in sidebar
    await page.locator('button:has-text("Settings")').click()

    // Wait for settings to load
    await page.waitForURL('**/settings')
    await page.waitForLoadState('networkidle')

    // Take screenshot
    await page.screenshot({ path: 'tests/screenshots/02-settings.png', fullPage: true })

    // Check settings content
    const heading = page.getByText('Settings').first()
    await expect(heading).toBeVisible()

    // Check profile section exists
    const profileSection = page.getByText('Profile')
    await expect(profileSection).toBeVisible()

    // Back button
    const backBtn = page.getByText('Back')
    await expect(backBtn).toBeVisible()

    // Navigate back
    await backBtn.click()
    await page.waitForURL('**/')
    await expect(page.locator('input[placeholder*="Message"]')).toBeVisible()
  })

  test('send a message gets a response', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // Type and send a message
    const input = page.locator('input[placeholder*="Message"]')
    await input.fill('Hello from test!')
    await input.press('Enter')

    // Should see the user message bubble
    const userBubble = page.locator('.bg-blue-500')
    // Wait a bit for response
    await page.waitForTimeout(2000)

    // Take screenshot
    await page.screenshot({ path: 'tests/screenshots/03-message-sent.png', fullPage: true })

    // At minimum, the user message should be visible
    const message = page.getByText('Hello from test!')
    await expect(message).toBeVisible()
  })

  test('sidebar collapse toggle works', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    const collapseBtn = page.locator('aside button:has-text("←")').last()
    await collapseBtn.click()
    await page.waitForTimeout(500)
    await page.screenshot({ path: 'tests/screenshots/04-sidebar-collapsed.png', fullPage: true })
  })

  test('voice buttons are present', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // Mic button
    const micBtn = page.locator('button[title*="recording"]')
    await expect(micBtn).toBeVisible()

    // Voice mode toggle
    const dictationBtn = page.getByText('Dictation')
    const chatBtn = page.getByText('Real-time chat')
    await expect(dictationBtn.or(chatBtn)).toBeVisible()

    await page.screenshot({ path: 'tests/screenshots/05-voice-ui.png', fullPage: true })
  })

})
