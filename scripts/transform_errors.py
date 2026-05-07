#!/usr/bin/env python3
"""Transform Rust error types from String to AppError across the codebase."""

import re
import sys
import os

SRC_DIR = sys.argv[1] if len(sys.argv) > 1 else "/Users/chen/Workspace/astro_studio/src-tauri/src"

def transform_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    original = content

    # 1. Replace .map_err(|e| e.to_string()) — remove entirely, rely on ? + From impl
    content = re.sub(r'\.map_err\(\|e\|\s*e\.to_string\(\)\)', '', content)

    # 2. Replace .map_err(|e| format!("msg: {}", e)) patterns for rusqlite errors
    #    These add context to the error message
    content = re.sub(
        r'\.map_err\(\|e\|\s*format!("([^"]+)", e)\)',
        r'.map_err(|e| AppError::Database { message: format!("\1", e) })',
        content
    )

    # 3. Handle .map_err with single-arg format where e is the only arg
    content = re.sub(
        r'\.map_err\(\|e\|\s*format!("([^"]*\{[^}]*\}[^"]*)", e)\)',
        r'.map_err(|e| AppError::Database { message: format!("\1", e) })',
        content
    )

    # 4. Handle .map_err(|e| format!("msg")) without e in format
    content = re.sub(
        r'\.map_err\(\|[a-z]+\|\s*format!("([^"]*)")\)',
        r'.map_err(|_e| AppError::Database { message: "\1".to_string() })',
        content
    )

    # 5. Replace Result<..., String> with Result<..., AppError> in fn signatures
    content = re.sub(
        r'-> Result<([^,>]+(?:<[^>]+>)?),\s*String>',
        r'-> Result<\1, AppError>',
        content
    )

    # 6. Replace Err("message".to_string()) with AppError::Validation
    content = re.sub(
        r'Err\("([^"]+)"\.to_string\(\)\)',
        r'Err(AppError::Validation { message: "\1".to_string() })',
        content
    )

    # 7. Replace .map_err(|e| e.to_string())? patterns (might have been missed)
    content = re.sub(r'\.map_err\(\|e\|\s*e\.to_string\(\)\)\?', '?', content)

    # 8. Add use crate::error::AppError; if Result<AppError> is present and not already imported
    if 'AppError' in content and 'use crate::error::AppError;' not in content:
        # Find the first use statement or mod declaration and add after
        lines = content.split('\n')
        insert_idx = 0
        for i, line in enumerate(lines):
            if line.startswith('use ') or line.startswith('mod '):
                insert_idx = i + 1
        if insert_idx > 0:
            # Find end of use block
            while insert_idx < len(lines) and (lines[insert_idx].startswith('use ') or lines[insert_idx].strip() == ''):
                if lines[insert_idx].strip() == '':
                    break
                insert_idx += 1
            lines.insert(insert_idx, 'use crate::error::AppError;')
            content = '\n'.join(lines)

    if content != original:
        with open(filepath, 'w') as f:
            f.write(content)
        return True
    return False

def main():
    rust_files = []
    for root, dirs, files in os.walk(SRC_DIR):
        for f in files:
            if f.endswith('.rs'):
                rust_files.append(os.path.join(root, f))

    changed = 0
    for f in sorted(rust_files):
        if transform_file(f):
            print(f"Transformed: {f}")
            changed += 1

    print(f"\nChanged {changed} files.")

if __name__ == '__main__':
    main()
