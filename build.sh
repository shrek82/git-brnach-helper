#!/bin/bash

# Git Helper TUI 构建和安装脚本
# 编译后的二进制文件名称为 gitbm

set -e

echo "=== 开始构建 gitbm ==="

# Release 模式编译
cargo build --release

echo "✅ 编译成功！"
echo ""

# 检查是否提供了安装参数
if [ "$1" == "--install" ] || [ "$1" == "-i" ]; then
    echo "正在安装到 /usr/local/bin/gitbm ..."
    sudo cp target/release/git-helper /usr/local/bin/gitbm
    echo ""
    echo "✅ 安装完成！"
    echo "   二进制文件：/usr/local/bin/gitbm"
    echo "   运行命令：gitbm"
else
    echo "二进制文件已生成：target/release/git-helper"
    echo ""
    echo "安装选项："
    echo "  ./build.sh --install   安装到 /usr/local/bin/gitbm (需要 sudo)"
    echo "  ./build.sh -i          同上 (简写)"
    echo ""
    echo "或者手动复制："
    echo "  sudo cp target/release/git-helper /usr/local/bin/gitbm"
fi
