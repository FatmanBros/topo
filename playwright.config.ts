import { defineConfig, devices } from '@playwright/test';
import { readFileSync, existsSync } from 'fs';

// Read basePath from topo.config.json
function getBasePath(): string {
  const configPath = './topo.config.json';
  if (existsSync(configPath)) {
    try {
      const config = JSON.parse(readFileSync(configPath, 'utf-8'));
      return config.build?.basePath || '';
    } catch {
      return '';
    }
  }
  return '';
}

const basePath = getBasePath();
const port = 3333;

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  use: {
    baseURL: `http://localhost:${port}`,
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: `topo start --port ${port} --no-open`,
    url: `http://localhost:${port}${basePath || '/'}`,
    reuseExistingServer: false,
    timeout: 120 * 1000,
  },
});
