import {Config} from '@remotion/cli/config';
import {existsSync} from 'node:fs';
Config.setVideoImageFormat('jpeg');
Config.setOverwriteOutput(true);
Config.setConcurrency(3);
Config.setChromeMode('chrome-for-testing');
const candidates=[process.env.REMOTION_BROWSER,'/Users/trip/Library/Caches/ms-playwright/chrome-mac/Chromium.app/Contents/MacOS/Chromium','/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'].filter(Boolean) as string[];
const browser=candidates.find(p=>existsSync(p));
if(browser) Config.setBrowserExecutable(browser);
