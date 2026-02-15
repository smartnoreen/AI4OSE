#!/bin/bash

echo "======================================"
echo "  准备推送到 GitHub"
echo "======================================"
echo ""

cd /home/coder/workspace/ch5测试

# 检查是否已有 origin
if git remote | grep -q origin; then
    echo "移除旧的远程仓库配置..."
    git remote remove origin
fi

echo "配置远程仓库 (SSH)..."
git remote add origin git@github.com:smartnoreen/AI4OSE.git

echo "重命名分支为 main..."
git branch -M main

echo ""
echo "======================================"
echo "  开始推送到 GitHub"
echo "======================================"
echo ""

# 推送到 GitHub
git push -u origin main

if [ $? -eq 0 ]; then
    echo ""
    echo "======================================"
    echo "  ✅ 成功推送到 GitHub!"
    echo "======================================"
    echo ""
    echo "访问你的仓库:"
    echo "https://github.com/smartnoreen/AI4OSE"
    echo ""
else
    echo ""
    echo "======================================"
    echo "  ❌ 推送失败"
    echo "======================================"
    echo ""
    echo "可能的原因:"
    echo "1. SSH 密钥未添加到 GitHub"
    echo "   访问: https://github.com/settings/keys"
    echo ""
    echo "2. 仓库 AI4OSE 不存在"
    echo "   访问: https://github.com/new 创建仓库"
    echo ""
    echo "3. 首次连接需要确认"
    echo "   运行: ssh -T git@github.com"
    echo ""
fi
