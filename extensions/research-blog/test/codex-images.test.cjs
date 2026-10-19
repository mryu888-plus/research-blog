'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { EventEmitter } = require('node:events');
const { PassThrough } = require('node:stream');
const { chatgptLogin, codexEnvironment, generationPrompt, readGeneratedImage, requestCodexImage, runCodex } = require('../src/codex-images.cjs');
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/l9sAAAAASUVORK5CYII=', 'base64');
const options = { codexPath: process.execPath, style: 'Clear schematic.', size: '1536x1024', quality: 'medium' };

test('Codex requires ChatGPT login and inherited API credentials cannot change billing', async () => {
  const env = codexEnvironment({ PATH: 'keep', CODEX_HOME: 'keep-auth-store', OPENAI_API_KEY: 'secret', CODEX_API_KEY: 'secret', CODEX_ACCESS_TOKEN: 'secret', OPENAI_BASE_URL: 'https://wrong', CODEX_THREAD_ID: 'parent' });
  assert.deepEqual(env, { PATH: 'keep', CODEX_HOME: 'keep-auth-store' });
  for (const status of ['Not logged in', 'Logged in using an API key']) await assert.rejects(chatgptLogin('fixture', undefined, async () => ({ code: 0, stdout: status, stderr: '' })), /ChatGPT/);
  await chatgptLogin('fixture', undefined, async () => ({ code: 0, stdout: '', stderr: 'Logged in using ChatGPT' }));
});
test('Codex generation quotes selected content, uses a private workspace and validates output before cleanup', async () => {
  const calls = []; let scratch;
  const runner = async (_binary, args, job) => {
    calls.push(args);
    if (args[0] === 'login') return { code: 0, stdout: 'Logged in using ChatGPT', stderr: '' };
    scratch = job.cwd;
    assert(!args.includes('--dangerously-bypass-approvals-and-sandbox'));
    assert.equal(args[args.indexOf('--sandbox') + 1], 'workspace-write');
    assert(args.includes('--ignore-user-config')); assert(args.includes('forced_login_method="chatgpt"'));
    assert(job.input.includes(JSON.stringify('词袋 "猫 猫 车"')));
    assert(job.input.includes('built-in image_gen')); assert(job.input.includes('Do NOT use an API key'));
    fs.writeFileSync(path.join(scratch, 'output.png'), PNG);
    fs.writeFileSync(path.join(scratch, 'result.json'), JSON.stringify({ status: 'generated', file: path.join(scratch, 'output.png'), detail: 'Done' }));
    return { code: 0 };
  };
  const result = await requestCodexImage(options, '词袋 "猫 猫 车"', undefined, { runner });
  assert.equal(calls.length, 2); assert.deepEqual(result.buffer, PNG); assert.equal(result.extension, 'png');
  assert.equal(fs.existsSync(scratch), false);
});
test('unavailable tools, fabricated output and bad images fail without returning an asset', async () => {
  for (const failure of ['unavailable', 'missing', 'bad-image', 'turn-failed']) {
    let scratch;
    await assert.rejects(requestCodexImage(options, '词袋', undefined, { runner: async (_binary, args, job) => {
      if (args[0] === 'login') return { code: 0, stdout: 'Logged in using ChatGPT', stderr: '' };
      scratch = job.cwd;
      fs.writeFileSync(path.join(scratch, 'result.json'), JSON.stringify({ status: failure === 'unavailable' ? 'unavailable' : 'generated', file: path.join(scratch, 'output.png'), detail: 'No image tool' }));
      if (failure === 'bad-image') fs.writeFileSync(path.join(scratch, 'output.png'), 'this is text, not an image');
      return failure === 'turn-failed' ? { code: 1, turnError: 'quota reached' } : { code: 0 };
    } }));
    assert.equal(fs.existsSync(scratch), false);
  }
});
test('cancellation after generation never delivers the image and removes scratch output', async () => {
  const controller = new AbortController(); let scratch;
  await assert.rejects(requestCodexImage(options, '词袋', controller.signal, { runner: async (_binary, args, job) => {
    if (args[0] === 'login') return { code: 0, stdout: 'Logged in using ChatGPT', stderr: '' };
    scratch = job.cwd; fs.writeFileSync(path.join(scratch, 'output.png'), PNG);
    controller.abort(); return { code: 0 };
  } }), /取消/);
  assert.equal(fs.existsSync(scratch), false);
});
test('oversized selections are rejected before starting any process', async () => {
  let called = false;
  await assert.rejects(requestCodexImage(options, 'x'.repeat(12001), undefined, { runner: async () => { called = true; } }), /12000/);
  assert.equal(called, false);
  assert(generationPrompt(options, 'example').includes('Do not copy files'));
});
test('paths outside image output roots and stale images cannot be returned as fresh generation', () => {
  assert.throws(() => readGeneratedImage(__filename, path.join(__dirname, 'scratch'), Date.now()), /路径/);
  assert.throws(() => readGeneratedImage('relative.png', __dirname, Date.now()), /完整路径/);
  assert.throws(() => readGeneratedImage(__filename, __dirname, Date.now() + 60000), /不是本次/);
});
test('a recovered connection warning does not turn a completed Codex run into a failure', async () => {
  const child = new EventEmitter(); child.stdout = new PassThrough(); child.stderr = new PassThrough(); child.stdin = new PassThrough();
  const pending = runCodex('fixture', ['exec'], { spawnImpl: (_binary, _args, config) => {
    assert.equal(config.shell, false); assert.equal(config.windowsHide, true); return child;
  } });
  child.stdout.write(JSON.stringify({ type: 'error', message: 'Reconnecting' }) + '\n');
  child.stdout.write(JSON.stringify({ type: 'turn.completed' }) + '\n'); child.emit('close', 0);
  const result = await pending; assert.equal(result.code, 0); assert.equal(result.turnError, '');
});
