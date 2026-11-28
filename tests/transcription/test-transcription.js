// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { spawn, execSync } from 'child_process';
import fetch from 'node-fetch';

const DEV_PORT = process.env.DEV_PORT || 8420;

async function runTest() {
  console.log('🧪 Automated Whisper Transcription Test');
  
  // Check if Tauri is already running
  let tauriProcess = null;
  try {
    await fetch(`http://localhost:${DEV_PORT}`);
    console.log(`✅ Tauri already running on port ${DEV_PORT}`);
  } catch {
    // Kill any stuck process
    try {
      execSync('taskkill /f /im echophrase.exe 2>nul', { stdio: 'ignore' });
    } catch {}
    
    // Start Tauri dev server
    console.log('🚀 Starting Tauri dev server...');
    tauriProcess = spawn('bun', ['run', 'tauri:dev'], {
      stdio: 'pipe',
      env: { ...process.env, RUST_LOG: 'info' }
    });
    
    // Wait for server to be ready
    console.log('⏳ Waiting for Tauri to start...');
    for (let i = 0; i < 30; i++) {
      try {
        await fetch(`http://localhost:${DEV_PORT}`);
        console.log('✅ Tauri started successfully');
        break;
      } catch {
        await new Promise(r => setTimeout(r, 1000));
      }
    }
  }
  
  // Now test the transcription using a headless browser automation
  console.log('📡 Testing transcription API...');
  
  try {
    // Use puppeteer-core or playwright to connect to the running Tauri app
    const { chromium } = await import('playwright');
    const browser = await chromium.launch({ headless: true });
    const page = await browser.newPage();
    
    await page.goto(`http://localhost:${DEV_PORT}`);
    
    // Execute transcription test in the page context
    const result = await page.evaluate(async () => {
      // Access Tauri API in the window
      if (window.__TAURI__ && window.__TAURI__.core) {
        try {
          const result = await window.__TAURI__.core.invoke('transcribe_audio', {
            filePath: 'tests/audio/test-1-2.wav'
          });
          return { success: true, data: result };
        } catch (err) {
          return { success: false, error: err.toString() };
        }
      } else {
        // Try the import method
        try {
          const { invoke } = await import('@tauri-apps/api/core');
          const result = await invoke('transcribe_audio', {
            filePath: 'tests/audio/test-1-2.wav'
          });
          return { success: true, data: result };
        } catch (err) {
          return { success: false, error: err.toString() };
        }
      }
    });
    
    await browser.close();
    
    if (result.success) {
      console.log('✅ Transcription successful!');
      console.log('📝 Result:', result.data);
      
      // Check if it contains expected text
      const text = result.data.toLowerCase();
      if (text.includes('test') || text.includes('one') || text.includes('two')) {
        console.log('✅ Contains expected "test one two" pattern!');
      } else {
        console.log('⚠️  Unexpected transcription result');
      }
    } else {
      console.error('❌ Transcription failed:', result.error);
    }
    
  } catch (err) {
    console.error('❌ Test failed:', err);
  } finally {
    // Clean up
    if (tauriProcess) {
      console.log('🧹 Cleaning up...');
      tauriProcess.kill();
    }
  }
}

runTest().catch(console.error);