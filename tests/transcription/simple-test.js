// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

// Simple automated test for transcription
import { execSync, spawn } from 'child_process';
import { existsSync } from 'fs';

console.log('🧪 Testing Whisper transcription automatically...');

// Check if test file exists
if (!existsSync('tests/audio/test-1-2.wav')) {
  console.error('❌ Test audio file not found!');
  process.exit(1);
}

// Kill any existing Tauri
try {
  execSync('taskkill /f /im echophrase.exe 2>nul', { stdio: 'ignore' });
} catch {}

console.log('🚀 Starting Tauri in test mode...');

// Start Tauri with a special test endpoint
const tauri = spawn('cargo', ['run', '--manifest-path', 'src-tauri/Cargo.toml', '--', '--test-transcribe'], {
  stdio: 'pipe',
  env: { ...process.env, RUST_LOG: 'info' }
});

let output = '';
let timeout;

tauri.stdout.on('data', (data) => {
  const str = data.toString();
  output += str;
  
  // Look for our debug logs
  if (str.includes('=== TRANSCRIBE DEBUG ===')) {
    console.log('📡 Transcription started...');
  }
  
  if (str.includes('Transcription result:') || str.includes('TEST_RESULT:')) {
    console.log('✅ Got transcription result!');
    console.log(str);
    clearTimeout(timeout);
    tauri.kill();
    process.exit(0);
  }
});

tauri.stderr.on('data', (data) => {
  console.error('[ERROR]', data.toString());
});

// Timeout after 30 seconds
timeout = setTimeout(() => {
  console.error('❌ Test timed out after 30 seconds');
  tauri.kill();
  process.exit(1);
}, 30000);

// Trigger test after Tauri starts
setTimeout(() => {
  console.log('🎯 Triggering transcription test...');
  // Send test command via stdin or another mechanism
}, 5000);