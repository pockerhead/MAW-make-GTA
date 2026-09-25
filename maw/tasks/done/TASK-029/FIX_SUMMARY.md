# FIX_SUMMARY — TASK-029 (tiny fixer)

Commit b814c9d on infra/test-ci.

Сначала проверил самое рискованное утверждение ревью: что с `if: success() || failure()` шаги "пропускаются при сбое setup". Это неверно. `failure()` истинно при провале любого предыдущего шага, включая setup, так что при сбое setup проверки тоже запустятся и упадут. Job всё равно будет красной, вреда нет, но формулировка ревью неточная.

## Fixed
1. Minor 1 (отмена прогонов на main): во всех 5 workflow `cancel-in-progress: ${{ github.ref != 'refs/heads/main' }}`.
2. Minor 2 (repo checks скрывают соседей): у шагов tree_check, font_check и test_brp стоит `if: success() || failure()`. Job по-прежнему краснеет, если упал любой из них.
3. Пробел "Test counts > 0": в sim, citygen и client в конец шага Test counts (он идёт с `if: always()`) добавлен awk, который делает `exit 1`, если сумма passed равна 0 или results.txt пустой. Локальный flip: на `0 passed` rc=1, на пустом файле rc=1.

## Skipped
- Minor 3 (дубли push/PR): оркестратор это не заказывал, PR пайплайн сейчас не открывает.

## Test results
YAML разбирается (PyYAML). CI на b814c9d: `gh run watch --exit-status` для 36081167184/266/185/252/281 дал rc=0. Все 5 success: repo checks, citygen gates, client gates, clippy, sim gates.
Flip-RED новой проверки в CI не делал. Проверен только локально на awk.
