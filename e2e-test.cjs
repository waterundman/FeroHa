#!/usr/bin/env node

/**
 * E2E测试脚本 - 验证贝叶斯笔记系统核心功能
 * 
 * 测试范围：
 * 1. 文件系统操作
 * 2. 向量数据库操作
 * 3. Dream Engine功能
 * 4. Skill管理器功能
 * 5. IPC协议功能
 */

const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');

// 测试结果收集
const testResults = {
  total: 0,
  passed: 0,
  failed: 0,
  errors: []
};

// 测试运行器
function runTest(name, testFn) {
  testResults.total++;
  try {
    testFn();
    testResults.passed++;
    console.log(`✓ ${name}`);
  } catch (error) {
    testResults.failed++;
    testResults.errors.push({ name, error: error.message });
    console.log(`✗ ${name}: ${error.message}`);
  }
}

// 断言函数
function assert(condition, message) {
  if (!condition) {
    throw new Error(message || 'Assertion failed');
  }
}

// 测试1: 检查Rust编译
function testRustCompilation() {
  const result = execSync('cargo check --manifest-path "D:\\新项目仓库\\贝叶斯笔记\\src-tauri\\Cargo.toml"', {
    encoding: 'utf-8',
    timeout: 60000
  });
  assert(!result.includes('error'), 'Rust compilation failed');
}

// 测试2: 运行Rust测试
function testRustTests() {
  const result = execSync('cargo test --manifest-path "D:\\新项目仓库\\贝叶斯笔记\\src-tauri\\Cargo.toml"', {
    encoding: 'utf-8',
    timeout: 120000
  });
  assert(result.includes('test result: ok'), 'Rust tests failed');
}

// 测试3: 运行前端测试
function testFrontendTests() {
  const result = execSync('npm test', {
    encoding: 'utf-8',
    timeout: 120000,
    cwd: 'D:\\新项目仓库\\贝叶斯笔记'
  });
  assert(result.includes('passed'), 'Frontend tests failed');
}

// 测试4: 检查关键文件存在
function testKeyFilesExist() {
  const keyFiles = [
    'src-tauri/src/ai/dream_engine.rs',
    'src-tauri/src/ai/skill_manager.rs',
    'src-tauri/src/ipc/protocol.rs',
    'src/components/InstructionCard.tsx',
    'src-tauri/src/fs/vault.rs',
    'src-tauri/src/ai/vectordb.rs'
  ];
  
  for (const file of keyFiles) {
    const filePath = path.join('D:\\新项目仓库\\贝叶斯笔记', file);
    assert(fs.existsSync(filePath), `Key file missing: ${file}`);
  }
}

// 测试5: 检查模块导出
function testModuleExports() {
  const modPath = path.join('D:\\新项目仓库\\贝叶斯笔记\\src-tauri\\src\\ai\\mod.rs');
  const content = fs.readFileSync(modPath, 'utf-8');
  assert(content.includes('pub mod dream_engine'), 'dream_engine module not exported');
  assert(content.includes('pub mod skill_manager'), 'skill_manager module not exported');
}

// 测试6: 检查IPC模块
function testIpcModule() {
  const modPath = path.join('D:\\新项目仓库\\贝叶斯笔记\\src-tauri\\src\\ipc\\mod.rs');
  const content = fs.readFileSync(modPath, 'utf-8');
  assert(content.includes('pub mod protocol'), 'protocol module not exported');
}

// 运行所有测试
console.log('=== 贝叶斯笔记 E2E测试 ===\n');

runTest('Rust编译检查', testRustCompilation);
runTest('Rust测试套件', testRustTests);
runTest('前端测试套件', testFrontendTests);
runTest('关键文件存在性', testKeyFilesExist);
runTest('AI模块导出', testModuleExports);
runTest('IPC模块导出', testIpcModule);

// 输出测试结果
console.log('\n=== 测试结果 ===');
console.log(`总计: ${testResults.total}`);
console.log(`通过: ${testResults.passed}`);
console.log(`失败: ${testResults.failed}`);

if (testResults.failed > 0) {
  console.log('\n失败详情:');
  testResults.errors.forEach(({ name, error }) => {
    console.log(`  - ${name}: ${error}`);
  });
  process.exit(1);
} else {
  console.log('\n✓ 所有E2E测试通过！');
  process.exit(0);
}
