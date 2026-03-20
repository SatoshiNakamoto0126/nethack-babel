## NetHack Babel — 简体中文消息目录
## Fluent (.ftl) 格式 — https://projectfluent.org/

## ============================================================================
## 战斗 — 近战
## ============================================================================

melee-hit-bare = { $attacker }击中了{ $defender }！
melee-hit-weapon = { $attacker }用{ $weapon }击中了{ $defender }！
melee-hit-slash = { $attacker }砍中了{ $defender }！
melee-hit-stab = { $attacker }刺中了{ $defender }！
melee-hit-bash = { $attacker }锤击了{ $defender }！
melee-hit-whip = { $attacker }鞭打了{ $defender }！
melee-hit-bite = { $attacker }咬了{ $defender }！
melee-hit-claw = { $attacker }抓伤了{ $defender }！
melee-hit-sting = { $attacker }蜇了{ $defender }！
melee-hit-butt = { $attacker }顶撞了{ $defender }！
melee-hit-kick = { $attacker }踢中了{ $defender }！
melee-miss = { $attacker }没有打中{ $defender }。
melee-miss-barely = { $attacker }差一点就打中{ $defender }了。
critical-hit = 暴击！
backstab = { $attacker }偷袭了{ $defender }！
joust-hit = { $attacker }用长矛冲刺了{ $defender }！
joust-lance-breaks = 你的长矛在冲击中碎裂了！
attack-blocked = { $defender }挡住了攻击。
attack-parried = { $defender }格开了攻击。

## ============================================================================
## 战斗 — 远程
## ============================================================================

ranged-hit = { $attacker }用{ $projectile }射中了{ $defender }！
ranged-miss = { $projectile }没有射中{ $defender }。
ranged-miss-wide = { $projectile }从{ $defender }身旁飞过。
throw-hit = { $projectile }击中了{ $defender }！
throw-miss = { $projectile }没有打中{ $defender }，落在了地上。
spell-hit = { $spell }命中了{ $defender }！
spell-miss = { $spell }没有命中{ $defender }。
spell-fizzle = 法术失效了。
wand-zap = 你挥动了{ $wand }！
wand-nothing = 什么也没有发生。
wand-wrested = 你从{ $wand }中榨出了最后一点能量。

## ============================================================================
## 战斗 — 伤害描述
## ============================================================================

damage-barely = 攻击几乎没有伤到{ $defender }。
damage-light = { $defender }受了轻伤。
damage-moderate = { $defender }受了中等程度的伤。
damage-heavy = { $defender }受了重伤。
damage-severe = { $defender }伤势严重。
damage-critical = { $defender }伤势危急！

## ============================================================================
## 战斗 — 被动伤害
## ============================================================================

passive-acid = 酸液灼烧着你！
passive-fire = 你被烧伤了！
passive-cold = 你被冻伤了！
passive-shock = 你被电击了！
passive-poison = 你感到恶心！
passive-drain = 你感到精力被吸走了！
passive-corrode = 你的{ $item }被腐蚀了！
passive-stun = 你被打得头昏眼花！
passive-slow = 你感到自己慢了下来！
passive-paralyze = 你被麻痹了！

## ============================================================================
## 战斗 — 死亡消息
## ============================================================================

entity-killed = { $entity }被杀死了！
entity-destroyed = { $entity }被摧毁了！
entity-dissolved = { $entity }溶解了！
entity-evaporates = { $entity }蒸发了！
entity-turns-to-dust = { $entity }化为了灰烬！
you = 你
you-hit-monster = 你击中了{ $monster }！
you-miss-monster = 你没有打中{ $monster }。
you-kill-monster = 你杀死了{ $monster }！
you-destroy-monster = 你摧毁了{ $monster }！
you-dissolve-monster = { $monster }溶解了！
monster-hits-you = { $monster }击中了你！
monster-misses-you = { $monster }没有打中你。
monster-kills-you = { $monster }杀死了你！
monster-turns-to-stone = { $monster }变成了石头！
monster-flees = { $monster }转身逃跑了！
monster-falls-asleep = { $monster }睡着了。

## ============================================================================
## 移动 — 门
## ============================================================================

door-opened = 门开了。
door-closed = 门关上了。
door-locked = 这扇门锁着。
door-broken = 你破开了门！
door-unlock = 你打开了门锁。
door-lock = 你锁上了门。
door-kick = 你踢了门一脚！
door-kick-fail = 砰！！！门纹丝不动。
door-resist = 门没有被打开！
door-jammed = 门卡住了。

## ============================================================================
## 移动 — 碰撞和地形
## ============================================================================

bump-wall = 你撞到了墙上。
bump-boulder = 你推动了巨石。
bump-boulder-fail = 巨石纹丝不动。
bump-closed-door = 哎哟！你撞到了一扇关着的门上。
bump-monster = 你撞上了{ $monster }。
swim-lava = 你正在熔岩中游泳！
swim-water = 你掉进了水里！
swim-sink = 你沉入了水面之下！
terrain-ice = 冰面很滑！
terrain-ice-slip = 你在冰上滑倒了！
terrain-mud = 你陷入了泥沼。

## ============================================================================
## 移动 — 陷阱
## ============================================================================

trap-triggered = 你触发了{ $trap }！
trap-disarmed = 你拆除了{ $trap }。
trap-pit-fall = 你掉进了一个坑里！
trap-spiked-pit = 你掉进了一个布满尖刺的坑里！
trap-arrow = 一支箭向你射来！
trap-dart = 一支小飞镖向你射来！
trap-bear = 你踩到了捕兽夹！
trap-teleport = 你感到一阵扭曲！
trap-level-teleport = 你感到一阵剧烈的变化！
trap-magic-portal = 你感到一阵眩晕。
trap-fire = 一根火柱喷发了！
trap-rolling-boulder = 一块巨石向你滚来！
trap-squeaky-board = 你脚下的木板发出吱嘎声。
trap-web = 你被蛛网缠住了！
trap-rust = 一股水流喷向你！
trap-polymorph = 你感到身体正在发生变化！

## ============================================================================
## 移动 — 楼梯和层级变化
## ============================================================================

stairs-up = 你走上楼梯。
stairs-down = 你走下楼梯。
stairs-nothing-up = 你没法在这里往上走。
stairs-nothing-down = 你没法在这里往下走。
level-change = 你进入了{ $level }。
level-feeling = 你对这一层有{ $feeling ->
    [good] 好的
    [bad] 不好的
   *[neutral] 不确定的
}预感。
level-feeling-objects = 你感觉这一层有贵重物品。
level-enter-shop = 你进入了{ $shopkeeper }的{ $shoptype }。
level-leave-shop = 你离开了商店。
elbereth-engrave = 你在地上刻下了"Elbereth"。
elbereth-warn = { $monster }看起来很害怕！
elbereth-fade = 刻文褪色了。

## ============================================================================
## 移动 — 环境交互
## ============================================================================

fountain-drink = 你喝了喷泉的水。
fountain-dry = 喷泉干涸了！
fountain-wish = 你看到一个闪光的水池。
sink-drink = 你喝了水龙头的水。
sink-ring = 你听到一枚戒指沿着排水管滚下去的声音。
altar-pray = 你开始向{ $god }祈祷。
altar-sacrifice = 你向{ $god }献上了{ $corpse }。
altar-desecrate = 你感到一股黑暗的力量。
throne-sit = 你坐在了王座上。
grave-dig = 你挖开了坟墓。
grave-disturb = 你打扰了{ $entity }的安息。

## ============================================================================
## 状态 — 饥饿
## ============================================================================

hunger-satiated = 你吃饱了。
hunger-not-hungry = 你不饿。
hunger-hungry = 你饿了。
hunger-weak = 你感到虚弱。
hunger-fainting = 你因为饥饿而昏倒了。
hunger-starved = 你饿死了。

## ============================================================================
## 状态 — 生命值和等级
## ============================================================================

hp-gained = 你感觉好了{ $amount ->
    [one] 一点
   *[other] 许多
}。
hp-lost = 你感觉更糟了{ $amount ->
    [one] 一点
   *[other] 许多
}。
hp-full = 你感觉完全恢复了。
hp-restored = 你的生命值恢复了。
level-up = 欢迎来到经验等级{ $level }！
level-down = 你感觉经验不如以前了。
level-max = 你感到无比强大！

## ============================================================================
## 状态 — 属性获得/失去
## ============================================================================

speed-gain = 你感到脚步轻快！
speed-lose = 你慢了下来。
speed-very-fast = 你感到速度飞快！
strength-gain = 你感到力大无穷！
strength-lose = 你感到力量减退！
telepathy-gain = 你感到一种奇异的心灵感应。
telepathy-lose = 你的感觉恢复了正常。
invisibility-gain = 你感到自己变得透明了！
invisibility-lose = 你重新变得可见了。
see-invisible-gain = 你感到洞察力增强了！
see-invisible-lose = 你的洞察力减弱了。
stealth-gain = 你感到自己的脚步更轻了！
stealth-lose = 你感到自己变得笨拙了。
fire-resist-gain = 你感到一阵凉意。
fire-resist-lose = 你感到温暖了。
cold-resist-gain = 你感到温暖。
cold-resist-lose = 你感到寒冷。
shock-resist-gain = 你感到自己被绝缘了。
shock-resist-lose = 你感到自己容易导电了。
poison-resist-gain = 你感到身体健康。
poison-resist-lose = 你感到身体不如以前健康了。

## ============================================================================
## 状态 — 异常状态
## ============================================================================

poison-affected = 你感觉{ $severity ->
    [mild] 有点不舒服
    [moderate] 生病了
   *[severe] 病得很重
}。
confusion-start = 你感到头晕目眩。
confusion-end = 你不再那么晕了。
blindness-start = 你什么也看不见了！
blindness-end = 你又能看见了。
stun-start = 你摇摇晃晃……
stun-end = 你感到站稳了一些。
hallucination-start = 哇哦！一切看起来都好迷幻！
hallucination-end = 你恢复了正常。
sleep-start = 你感到昏昏欲睡……
sleep-end = 你醒了过来。
petrification-start = 你正在变成石头！
petrification-cure = 你感到身体灵活了一些。
lycanthropy-start = 你感到发烧了。
lycanthropy-cure = 你感觉好多了。
levitation-start = 你飘了起来！
levitation-end = 你缓缓降落。

## ============================================================================
## 状态 — 负重
## ============================================================================

encumbrance-unencumbered = 你行动自如。
encumbrance-burdened = 你负担较重。
encumbrance-stressed = 你负担很重。
encumbrance-strained = 你已经不堪重负！
encumbrance-overtaxed = 你已经超负荷了！
encumbrance-overloaded = 你负重过大，无法移动！

## ============================================================================
## 物品 — 拾取和丢弃
## ============================================================================

item-picked-up = { $actor }捡起了{ $item }。
item-dropped = { $actor }放下了{ $item }。
you-pick-up = 你捡起了{ $item }。
you-drop = 你放下了{ $item }。
you-pick-up-gold = 你捡起了{ $amount }枚金币。
nothing-to-pick-up = 这里没有东西可以捡。
too-many-items = 你的东西太多了！

## ============================================================================
## 物品 — 挥舞武器
## ============================================================================

item-wielded = { $actor }装备了{ $item }。
you-wield = 你挥舞起{ $weapon }。
    .two-handed = （双手武器）
you-wield-already = 你已经在使用那件武器了！
you-unwield = 你收起了{ $weapon }。
you-wield-nothing = 你空着双手。
weapon-weld-cursed = { $weapon }粘在了你的手上！

## ============================================================================
## 物品 — 护甲
## ============================================================================

item-worn = { $actor }穿上了{ $item }。
item-removed = { $actor }脱下了{ $item }。
you-wear = 你穿上了{ $armor }。
you-remove = 你脱下了{ $armor }。
you-remove-cursed = 你脱不下{ $armor }。它被诅咒了！
armor-crumbles = 你的{ $armor }碎裂成灰了！

## ============================================================================
## 物品 — 鉴定和状态
## ============================================================================

item-identified = 你知道了{ $item }就是{ $identity }。
item-damaged = { $item }受损了！
item-destroyed = { $item }被摧毁了！
item-cursed = { $item }被诅咒了！
item-blessed = { $item }被祝福了。
item-enchanted = { $item }闪了一下{ $color }的光。
item-rusted = { $item }生锈了。
item-burnt = { $item }被烧焦了！
item-rotted = { $item }腐烂了。
item-corroded = { $item }被腐蚀了。
item-eroded-away = { $item }完全腐蚀掉了！

## ============================================================================
## 物品 — 食物和饮食
## ============================================================================

eat-start = 你开始吃{ $food }。
eat-finish = 你吃完了{ $food }。
eat-delicious = 真好吃！
eat-disgusting = 呕！真难吃！
eat-poisoned = 呸——一定是有毒的！
eat-rotten = 呃——这食物腐烂了！
eat-cannibal = 你这个食人者！你感到致命的恶心。
eat-corpse-old = 这具{ $monster }的尸体有点不新鲜了。

## ============================================================================
## 物品 — 药水和卷轴
## ============================================================================

potion-drink = 你喝了{ $potion }。
potion-shatter = { $potion }碎了！
potion-boil = { $potion }沸腾蒸发了。
potion-freeze = { $potion }结冰碎裂了！
scroll-read = 你读卷轴的时候，它消失了。
scroll-blank = 这张卷轴似乎是空白的。
scroll-cant-read = 你看不见，没法阅读！
spellbook-read = 你开始研读{ $spellbook }。
spellbook-learn = 你学会了{ $spell }法术！
spellbook-forget = 你忘记了{ $spell }法术。
spellbook-too-hard = 这本魔法书对你来说太难了。

## ============================================================================
## 物品 — 金币
## ============================================================================

gold-pick-up = 你捡起了{ $amount }枚金币。
gold-drop = 你丢下了{ $amount }枚金币。
gold-paid = 你支付了{ $amount }枚金币。
gold-received = 你收到了{ $amount }枚金币。

## ============================================================================
## 物品 — 容器
## ============================================================================

container-open = 你打开了{ $container }。
container-close = 你关上了{ $container }。
container-empty = { $container }是空的。
container-locked = { $container }锁着。
container-trap = 你触发了{ $container }上的陷阱！
container-looted = 你搜索了{ $container }。

## ============================================================================
## 怪物 — 行动
## ============================================================================

monster-moves = { $monster }移动了。
monster-picks-up = { $monster }捡起了{ $item }。
monster-wields = { $monster }挥舞起{ $item }！
monster-wears = { $monster }穿上了{ $item }。
monster-eats = { $monster }吃了{ $food }。
monster-drinks = { $monster }喝了{ $potion }！
monster-casts = { $monster }施放了法术！
monster-breathes = { $monster }喷出了{ $element }！
monster-summons = { $monster }召唤了援军！
monster-steals = { $monster }偷走了{ $item }！
monster-grabs = { $monster }抓住了你！
monster-throws = { $monster }投掷了{ $item }！
monster-zaps = { $monster }挥动了{ $wand }！

## ============================================================================
## 怪物 — 声音
## ============================================================================

sound-growl = 你听到一阵低沉的咆哮。
sound-roar = 你听到一声怒吼！
sound-hiss = 你听到嘶嘶声！
sound-buzz = 你听到嗡嗡声。
sound-chug = 你听到咕咚咕咚的声音。
sound-splash = 你听到水花声。
sound-clank = 你听到铿锵声。
sound-scream = 你听到一声惨叫！
sound-squeak = 你听到吱吱声。
sound-laugh = 你听到疯狂的笑声！
sound-wail = 你听到哀嚎声。
sound-whisper = 你听到窃窃私语。
sound-coins = 你听到金币叮当作响。
sound-footsteps = 你听到脚步声。
sound-digging = 你听到挖掘声。

## ============================================================================
## 怪物 — 宠物消息
## ============================================================================

pet-eats = 你的{ $pet }吃了{ $food }。
pet-drops = 你的{ $pet }放下了{ $item }。
pet-picks-up = 你的{ $pet }捡起了{ $item }。
pet-whimper = 你的{ $pet }呜咽着。
pet-happy = 你的{ $pet }看起来很高兴。
pet-wag = 你的{ $pet }摇了摇尾巴。
pet-hostile = 你的{ $pet }变得狂暴了！
pet-tame = 你驯服了{ $monster }。
pet-name = 你想给你的{ $pet }起什么名字？

## ============================================================================
## 界面 — 提示
## ============================================================================

more-prompt = ——继续——
quit-prompt = 你确定要退出吗？
really-quit = 真的要退出吗？
prompt-direction = 朝哪个方向？
prompt-eat = 你想吃什么？
prompt-drink = 你想喝什么？
prompt-read = 你想阅读什么？
prompt-zap = 你想使用哪根魔杖？
prompt-throw = 你想投掷什么？
prompt-name = 你想命名什么？
prompt-call = 称之为：
prompt-confirm = 你确定吗？[yn]
prompt-pay = 花{ $amount }购买{ $item }？

## ============================================================================
## 界面 — 游戏结束和得分
## ============================================================================

game-over = 你死了。分数：{ $score }。
game-over-escaped = 你逃出了地牢！
game-over-ascended = 你飞升成为了半神！
game-over-quit = 你退出了游戏。
game-over-possessions = 你想鉴定你的物品吗？
game-over-topten = 你进入了前十名！
game-over-score-final = 最终分数：{ $score }。
game-over-turns = 你坚持了{ $turns }个回合。
game-over-killer = 死因：{ $killer }。
game-over-epitaph = 安息吧，{ $name }。
gender-male = 男
gender-female = 女
gender-neuter = 无性
death-cause-killed-by = 被{ $killer }杀死
death-cause-starvation = 饿死
death-cause-poisoning = 死于中毒
death-cause-petrification = 变成了石头
death-cause-drowning = 溺死
death-cause-burning = 被烧死
death-cause-disintegration = 被裂解了
death-cause-sickness = 病死
death-cause-strangulation = 被勒死
death-cause-falling = 摔死
death-cause-crushed-boulder = 被巨石压死
death-cause-quit = 主动退出
death-cause-escaped = 成功逃脱
death-cause-ascended = 成功飞升
death-cause-trickery = 死于诡计
ui-tombstone-epitaph = { $name }，{ $level }级冒险者
ui-tombstone-info = { $cause } | 分数：{ $score } | 回合：{ $turns } | HP：{ $hp }/{ $maxhp }

## ============================================================================
## 界面 — 欢迎和状态
## ============================================================================

welcome = 欢迎来到 NetHack Babel，{ $role }{ $name }！
welcome-back = 欢迎回到 NetHack Babel，{ $role }{ $name }！
character-description = { $race }{ $role }{ $name }
dungeon-level = 地牢第{ $depth }层
status-line = 生命:{ $hp }/{ $maxhp } 魔力:{ $pw }/{ $maxpw } 防御:{ $ac } 经验:{ $level }

## ============================================================================
## 界面 — 帮助
## ============================================================================

help-title = NetHack Babel 帮助
help-move = 使用 hjklyubn 或方向键移动。
help-attack = 走向怪物即可攻击。
help-wait = 按 . 或 s 等待一回合。
help-search = 按 s 搜索隐藏的东西。
help-inventory = 按 i 查看背包。
help-pickup = 按 , 拾取物品。
help-drop = 按 d 丢弃物品。
help-stairs-up = 按 < 上楼。
help-stairs-down = 按 > 下楼。
help-eat = 按 e 进食。
help-quaff = 按 q 喝药水。
help-read = 按 r 阅读卷轴或魔法书。
help-wield = 按 w 挥舞武器。
help-wear = 按 W 穿戴护甲。
help-remove = 按 T 脱下护甲。
help-zap = 按 z 使用魔杖。

# 帮助 — 移动图示
help-move-diagram =
    {"  "}y k u     左上 上 右上
    {"  "}h . l      左  .  右
    {"  "}b j n     左下 下 右下

# 帮助 — 符号
help-symbols-title = 符号说明：
help-symbol-player = @  = 你（玩家）
help-symbol-floor = .  = 地面
help-symbol-corridor = #  = 走廊
help-symbol-door-closed = +  = 关闭的门
help-symbol-door-open = |  = 打开的门
help-symbol-stairs-up = <  = 上楼梯
help-symbol-stairs-down = >  = 下楼梯
help-symbol-water = {"}"}  = 水/岩浆
help-symbol-fountain = {"{"} = 喷泉

# 帮助 — 附加命令
help-options = 按 O 打开设置。
help-look = 按 : 查看地面。
help-history = 按 Ctrl+P 查看消息历史。
help-shift-run = Shift+方向键 = 沿方向奔跑。
help-arrows = 方向键也可用于移动。

## ============================================================================
## 系统 — 存档和读档
## ============================================================================

save-game = 正在保存游戏……
save-complete = 游戏已保存。
save-failed = 保存失败！
load-game = 正在恢复存档……
load-complete = 游戏已恢复。
load-failed = 无法恢复存档。
load-version-mismatch = 存档版本不匹配。

## ============================================================================
## 系统 — 配置
## ============================================================================

config-loaded = 配置已加载。
config-error = 配置错误：{ $message }。
config-option-set = 选项"{ $option }"已设置为"{ $value }"。
config-option-unknown = 未知选项：{ $option }。
config-language-set = 语言已设置为{ $language }。
config-language-unknown = 未知语言：{ $language }。

## ============================================================================
## 系统 — 错误消息
## ============================================================================

error-generic = 出了点问题。
error-save-corrupt = 存档已损坏。
error-out-of-memory = 内存不足！
error-file-not-found = 找不到文件：{ $file }。
error-permission = 权限被拒绝。
error-panic = 严重错误：{ $message }
error-impossible = 程序异常：{ $message }

## ============================================================================
## 其他 — 查看和搜索
## ============================================================================

nothing-here = 这里什么都没有。
something-here = 你看到这里有{ $item }。
several-things = 这里有好几样东西。
look-position = 你看到了{ $description }。
search-nothing = 你什么也没找到。
search-found = 你找到了{ $item }！
search-secret-door = 你发现了一扇暗门！
search-secret-passage = 你发现了一条暗道！
search-trap = 你发现了{ $trap }！

## ============================================================================
## 其他 — 负重和触及
## ============================================================================

cannot-reach = 你够不到那个。
too-heavy = { $item }太重了。
inventory-full = 你的背包满了。
pay-prompt = 花{ $amount }购买{ $item }？
cannot-do-while-blind = 你看不见，没法那样做。
cannot-do-while-confused = 你太晕了。
cannot-do-while-stunned = 你太眩晕了。
cannot-do-while-hallu = 你现在无法集中精神。

## ============================================================================
## 物品 — 卷轴（阅读效果）
## ============================================================================

scroll-dust = 你阅读的时候，卷轴化为了灰烬。
scroll-enchant-weapon = 你的{ $weapon }闪了一下{ $color }的光。
scroll-enchant-armor = 你的{ $armor }闪了一下{ $color }的光。
scroll-identify = 你感到更有见识了。
scroll-identify-prompt = 你想鉴定什么？
scroll-identify-all = 你鉴定了背包里的所有物品！
scroll-remove-curse = 你感到有人在帮助你。
scroll-remove-curse-nothing = 你没有感到任何不同。
scroll-teleport = 你感到一阵扭曲。
scroll-teleport-no-effect = 你感到短暂的迷失。
scroll-create-monster = 你听到低沉的嗡嗡声。
scroll-scare-monster = 你听到远处传来疯狂的笑声。
scroll-confuse-monster = 你的双手开始发出{ $color }的光。
scroll-magic-mapping = 你看到了地牢的全貌！
scroll-fire = 卷轴爆发出一根火柱！
scroll-earth = 大地在你脚下震动！
scroll-amnesia = 你感觉有些东西被遗忘了。
scroll-punishment = 你因为不良行为被惩罚了！
scroll-stinking-cloud = 一团恶臭的云雾从卷轴中涌出。
scroll-charging = 你感到一股魔力涌入。
scroll-genocide = 一个雷鸣般的声音在洞穴中回荡！
scroll-light = 一道光芒充满了房间！
scroll-food-detection = 你感觉到食物的存在。
scroll-gold-detection = 你感觉到金子的存在。
scroll-destroy-armor = 你的{ $armor }碎裂成灰了！
scroll-taming = 你感到魅力非凡。
scroll-mail = 你收到了一张信件卷轴。
scroll-blank-paper = 这张卷轴看起来是空白的。

## ============================================================================
## 物品 — 魔杖（使用效果）
## ============================================================================

wand-fire = 一道火焰从{ $wand }中射出！
wand-cold = 一道寒气从{ $wand }中射出！
wand-lightning = 一道闪电从{ $wand }中射出！
wand-magic-missile = 一颗魔法飞弹从{ $wand }中射出！
wand-sleep = 一道催眠射线从{ $wand }中射出。
wand-death = 一道死亡射线从{ $wand }中射出！
wand-polymorph = 一道闪烁的射线从{ $wand }中射出。
wand-striking = 一道光束从{ $wand }中射出！
wand-slow = 一道减速射线从{ $wand }中射出。
wand-speed = 一道加速光束从{ $wand }中射出。
wand-undead-turning = 一道驱逐亡灵的光束从{ $wand }中射出。
wand-opening = 一道开启光束从{ $wand }中射出。
wand-locking = 一道锁定光束从{ $wand }中射出。
wand-probing = 一道探测射线从{ $wand }中射出。
wand-digging = 一道挖掘光束从{ $wand }中射出！
wand-teleportation = 一道传送光束从{ $wand }中射出。
wand-create-monster = 你听到低沉的嗡嗡声。
wand-cancellation = 一道消除光束从{ $wand }中射出！
wand-make-invisible = 一道隐形光束从{ $wand }中射出。
wand-light = 一阵光芒从{ $wand }中涌出！
wand-darkness = 一团黑暗从{ $wand }中涌出。
wand-wishing = 你可以许一个愿望。
wand-ray-reflect = 光束从{ $surface }上反射了！
wand-ray-bounce = 光束从墙上弹开了！
wand-ray-absorb = { $entity }吸收了射线。
wand-break = { $wand }碎裂并爆炸了！
wand-break-effect = 一团{ $element }涌了出来！
wand-charge-empty = { $wand }似乎没有剩余能量了。
wand-recharge = { $wand }闪了一下光。
wand-recharge-fail = { $wand }剧烈震动后爆炸了！
wand-turn-to-dust = { $wand }化为了灰烬。

## ============================================================================
## 物品 — 药水（饮用效果）
## ============================================================================

potion-healing = 你感觉好了一些。
potion-extra-healing = 你感觉好多了！
potion-full-healing = 你感觉完全恢复了！
potion-gain-ability = 你感到自己的{ $stat ->
    [str] 力量
    [dex] 敏捷
    [con] 体质
    [int] 智力
    [wis] 智慧
   *[cha] 魅力
}增强了！
potion-gain-level = 你升了上去，穿过了天花板！
potion-gain-energy = 魔法能量在你体内流动！
potion-speed = 你感到速度飞快！
potion-invisibility = 你感到自己变得透明了！
potion-see-invisible = 你感到洞察力增强了！
potion-levitation = 你开始漂浮在空中！
potion-confusion = 哈？什么？我在哪？
potion-blindness = 一切都变暗了！
potion-hallucination = 哇哦！一切看起来都好迷幻！
potion-sleeping = 你感到非常困倦。
potion-paralysis = 你动不了了！
potion-poison = 你感到非常不舒服。
potion-acid = 酸液灼烧着你！
potion-oil = 那真顺滑。
potion-water = 这尝起来像水。
potion-holy-water = 你感到被净化了。
potion-unholy-water = 你感到一股邪恶的气息包围着你。
potion-object-detection = 你感觉到物品的存在。
potion-monster-detection = 你感觉到怪物的存在。
potion-sickness = 你呕吐了。
potion-restore-ability = 你感到力量恢复了！
potion-polymorph = 你感到身体正在发生变化。
potion-booze = 呃！这尝起来像{ $liquid }！
potion-fruit-juice = 这尝起来像{ $fruit }汁。
potion-mix = 药水混合后产生了{ $result }。
potion-dilute = { $potion }变稀了。
potion-vapor = 你吸入了一股{ $effect }气息。

## ============================================================================
## 物品 — 物品交互消息
## ============================================================================

pickup-with-quantity = 你捡起了{ $quantity }个{ $item }。
drop-with-quantity = 你放下了{ $quantity }个{ $item }。
cannot-carry-more = 你没法再拿更多了。
knapsack-full = 你的背包装不下更多东西了。
encumbrance-prevents = 你负重太大，没法做那个。
encumbrance-warning-burdened = 负重拖慢了你的脚步。
encumbrance-warning-stressed = 你在沉重的负担下步履蹒跚。
encumbrance-warning-strained = 你几乎无法在这样的负重下移动！
encumbrance-warning-overtaxed = 你就要在负重下倒下了！
identify-result = { $item }是{ $identity }。
identify-already-known = 你已经知道了。
altar-buc-blessed = { $item }发出明亮的琥珀色光芒！
altar-buc-uncursed = { $item }发出微弱的琥珀色光芒。
altar-buc-cursed = { $item }笼罩着黑色的光晕。
altar-buc-unknown = 似乎什么也没有发生。
item-name-prompt = 你想给{ $item }起什么名字？
item-called-prompt = 你想怎么称呼{ $item_class }？
item-name-set = 你给{ $item }命名为"{ $name }"。
item-called-set = 你把{ $item_class }称为"{ $name }"。
nothing-to-drop = 你没有东西可以丢弃。
nothing-to-eat = 你没有东西可以吃。
nothing-to-drink = 你没有东西可以喝。
nothing-to-read = 你没有东西可以阅读。
nothing-to-wield = 你没有东西可以挥舞。
nothing-to-wear = 你没有东西可以穿戴。
nothing-to-remove = 你没有穿戴任何东西可以脱下。
nothing-to-zap = 你没有东西可以使用。
nothing-to-throw = 你没有东西可以投掷。
nothing-to-apply = 你没有东西可以使用。

## ============================================================================
## 战斗 — 远程和投掷
## ============================================================================

throw-weapon = 你投掷了{ $weapon }！
throw-hits-wall = { $projectile }撞到了墙上。
throw-falls-short = { $projectile }没有飞到目标处。
throw-lands = { $projectile }落在了地上。
throw-breaks = { $projectile }碎裂了！
throw-multishot = 你射出了{ $count }发{ $projectile }！
throw-boomerang-return = { $projectile }飞回了你的手中！
throw-boomerang-miss = { $projectile }没有飞回来！
ranged-ammo-break = { $projectile }碎了！
ranged-ammo-lost = { $projectile }不见了。
ranged-quiver-empty = 你的箭袋空了。
ranged-no-ammo = 你没有合适的弹药。
ranged-not-ready = 你还没准备好发射。
shoot-hit = { $projectile }击中了{ $defender }！
shoot-miss = { $projectile }从{ $defender }身旁飞过。
shoot-kill = { $projectile }摧毁了{ $defender }！
multishot-fire = 你向{ $defender }射出了{ $count }发{ $projectile }！
launcher-wield = 你举起了{ $launcher }。
launcher-no-ammo = 你没有{ $launcher }的弹药。

## ============================================================================
## 物品 — 戒指和护身符
## ============================================================================

ring-put-on = 你戴上了{ $ring }。
ring-remove = 你摘下了{ $ring }。
ring-cursed-remove = { $ring }被诅咒了！你无法摘下它。
ring-effect-gain = 你感到{ $effect }。
ring-effect-lose = 你不再感到{ $effect }。
ring-shock = 戒指电击了你！
ring-hunger = 你感到一阵饥饿。
ring-sink-vanish = { $ring }从你的手指上滑落，消失在排水口中！
amulet-put-on = 你戴上了{ $amulet }。
amulet-remove = 你摘下了{ $amulet }。
amulet-cursed = { $amulet }粘在了你的脖子上！
amulet-strangulation = 护身符勒住了你的脖子！
amulet-lifesave = 你的护身符碎成了碎片！
amulet-lifesave-msg = 但是等等……你的护身符变得温暖了！
amulet-reflection = { $attack }被你的护身符反射了！

## ============================================================================
## 物品 — 工具
## ============================================================================

tool-apply = 你使用了{ $tool }。
tool-lamp-on = { $lamp }开始发光了。
tool-lamp-off = { $lamp }熄灭了。
tool-lamp-fuel = { $lamp }的燃料用完了。
tool-pick-locked = 你成功撬开了锁。
tool-pick-fail = 你没能撬开锁。
tool-horn-blow = 你吹出了一声{ $effect }！
tool-mirror-reflect = 你用镜子反射了{ $attack }！
tool-mirror-look = 你在镜子里看到了自己。
tool-stethoscope = 你听诊了{ $target }。
tool-tinning-kit = 你开始将{ $corpse }做成罐头。
tool-leash-attach = 你把{ $leash }系在了{ $pet }身上。
tool-leash-detach = 你解开了{ $pet }身上的{ $leash }。
tool-camera-flash = 你对{ $target }拍了张照！
tool-whistle-blow = 你吹了哨子。
tool-whistle-magic = 你吹出了一种奇异的哨音！

## ============================================================================
## 地牢设施 — 特殊房间和事件
## ============================================================================

shop-enter = 你进入了{ $shopkeeper }的{ $shoptype }。
shop-leave = { $shopkeeper }说"欢迎再来！"
shop-price = 「给你，{ $item }只要{ $price }枚金币。」
shop-price-bargain = 「给你，{ $item }只要{ $price }枚金币，真是便宜。」
shop-price-excellent-choice = 「给你，{ $item }只要{ $price }枚金币，绝对是上佳之选。」
shop-price-finest-quality = 「给你，{ $item }只要{ $price }枚金币，品质上乘。」
shop-price-gourmets-delight = 「给你，{ $item }只要{ $price }枚金币，美食家的最爱！」
shop-price-painstakingly-developed = 「给你，{ $item }只要{ $price }枚金币，精心打造！」
shop-price-superb-craftsmanship = 「给你，{ $item }只要{ $price }枚金币，做工精妙！」
shop-price-one-of-a-kind = 「给你，{ $item }只要{ $price }枚金币，独一无二！」
shop-stolen = 你有未付款的商品！
shop-enter-digging-tool = 店里传来警告，让你把挖掘工具留在外面。
shop-enter-steed = 店里传来声音，坚持要你把{ $steed }留在外面。
shop-enter-invisible = 店里传来怀疑的声音：隐形顾客不受欢迎。
shop-leave-warning = { $shopkeeper }喊道："请先付款再离开！"
shop-damage = { $shopkeeper }说"你得赔偿损失！"
shop-repair = { $shopkeeper }开始修理店里的损坏。
shop-keeper-dead = { $shopkeeper }死了，这家店已经废弃。
shop-shoplift = { $shopkeeper }尖叫道："站住，小偷！"
temple-enter = 你进入了{ $god }的神殿。
temple-forbidding = 你感到一股令人敬畏又排斥的神圣气息。
temple-peace = 一股深沉的平静笼罩着这座神殿。
temple-unusual-peace = 这座神殿显得异常平静。
temple-donate = { $priest }接受了你的捐赠。
temple-protection = { $priest }赐予你神圣的保护。
vault-guard = 突然，一名金库守卫出现了！
vault-guard-ask = 「你是谁？你在这里做什么？」

## ============================================================================
## 陷阱 — 扩展陷阱消息
## ============================================================================

trap-bear-leg = 捕兽夹夹住了你的腿！
trap-bear-escape = 你从捕兽夹中挣脱了。
trap-bear-stuck = 你被捕兽夹困住了！
trap-pit-climb = 你爬出了坑。
trap-pit-cant-climb = 你试图爬出坑，但失败了！
trap-spiked-damage = 那些尖刺是有毒的！
trap-arrow-dodge = 你躲开了那支箭！
trap-dart-poison = 那支飞镖是有毒的！
trap-land-mine = 轰！！你触发了一枚地雷！
trap-land-mine-set = 你安置了地雷。
trap-sleeping-gas = 一团毒气让你昏睡了！
trap-hole = 你从地板上的洞掉了下去！
trap-trapdoor = 你脚下的活板门突然打开了！
trap-magic-trap = 你被魔法爆炸吞没了！
trap-anti-magic = 你感到魔力被抽空了！
trap-statue = 雕像活了过来！
trap-vibrating-square = 你感到脚下有一种奇异的振动。
trap-seen = 你看到这里有{ $trap }。
trap-monster-trigger = { $monster }触发了{ $trap }！
trap-monster-pit = { $monster }掉进了坑里！
trap-monster-bear = { $monster }被捕兽夹夹住了！
trap-monster-web = { $monster }被蛛网缠住了！
trap-monster-teleport = { $monster }消失了！
trap-set-fail = 你没能安置好陷阱。
trap-set-success = 你安置了{ $trap }。

## ============================================================================
## 神器 — 特殊效果和消息
## ============================================================================

artifact-resist = 神器进行了抵抗！
artifact-evade = { $artifact }躲开了你的触碰！
artifact-blast = { $artifact }灼伤了你！
artifact-glow-fire = { $artifact }散发着神圣的火焰！
artifact-glow-cold = { $artifact }散发着冰蓝色的光芒！
artifact-glow-warning = { $artifact }发出警示的光芒！
artifact-invoke = 你唤起了{ $artifact }的力量。
artifact-invoke-fail = 似乎什么也没有发生。
artifact-gift = { $god }赐予你{ $artifact }！
artifact-touch-blast = { $artifact }灼烧了你的肌肤！
artifact-speak = { $artifact }对你说话了！
artifact-sing = { $artifact }在你手中吟唱着。
artifact-thirst = { $artifact }渴望鲜血！
artifact-kill-msg = { $artifact }以致命的力量击中了{ $defender }！
artifact-bisect = { $artifact }将{ $defender }劈成了两半！
artifact-drain-life = { $artifact }吸取了{ $defender }的生命力！
artifact-found = 你感觉到附近有{ $artifact }的存在。
artifact-already-exists = 这个名字的神器已经存在于这局游戏中了。
artifact-wish-denied = 你感到手中出现了什么东西，但随即消失了！
artifact-name-change = { $artifact }的名字在你眼前变化了！

## ============================================================================
## 商店 — 扩展消息
## ============================================================================

shop-owe = 你欠{ $shopkeeper }{ $amount }枚金币。
shop-bill-total = 你的账单总计{ $amount }枚金币。
shop-pay-success = 你向{ $shopkeeper }支付了{ $amount }枚金币。
shop-usage-fee = { $shopkeeper }说道：“使用费，{ $amount }枚金币。”
shop-no-money = 你没有足够的钱！
shop-buy = 你花了{ $price }枚金币买了{ $item }。
shop-sell = 你以{ $price }枚金币的价格卖出了{ $item }。
shop-credit = 你有{ $amount }枚金币的信用额度。
shop-door-block = { $shopkeeper }挡住了门口！
shop-angry = { $shopkeeper }发怒了！
shop-kops = 基石警察来了！
shop-kops-arrive = 基石警察赶到了！
shop-use-unpaid = { $shopkeeper }喊道："你在使用未付款的商品！"
shop-broke-item = 你弄坏了{ $item }！{ $shopkeeper }要求你赔偿{ $price }枚金币。
shop-welcome-back = { $shopkeeper }说"欢迎回来！你欠了{ $amount }枚金币。"
shop-closed = 商店关门了。

## ============================================================================
## 宗教 — 祈祷、祭祀、加冕
## ============================================================================

pray-start = 你开始向{ $god }祈祷。
pray-feel-warm = 你感到一阵温暖的光辉。
pray-feel-at-peace = 你感到内心平静。
pray-full-heal = 你感觉好多了！
pray-uncurse = 你感觉{ $god }正在帮助你。
pray-resist = 你感到获得了抗性！
pray-angry-god = { $god }很不高兴！
pray-ignored = { $god }似乎没有在听。
pray-punish = { $god }惩罚了你！
pray-gift-weapon = { $god }赐予了你一件礼物！
pray-mollified = { $god }似乎不那么生气了。
pray-reconciled = { $god }似乎已经原谅了你。
sacrifice-accept = 你的祭品在火焰中被吞噬了！
sacrifice-reject = { $god }不为所动。
sacrifice-already-full = 你有一种不够格的感觉。
sacrifice-wrong-altar = 你感到愧疚。
sacrifice-convert = 祭坛转化为{ $god }的了！
sacrifice-gift = { $god }很高兴，赐予你一件礼物！
crown-msg = 你听到一个声音在回荡："汝乃天选之人！"
crown-gain = 你感到{ $god }的力量在体内涌动！

## ============================================================================
## 宠物 — 扩展消息
## ============================================================================

pet-hungry = 你的{ $pet }看起来很饿。
pet-very-hungry = 你的{ $pet }非常饿！
pet-starving = 你的{ $pet }快饿死了！
pet-refuses-food = 你的{ $pet }拒绝吃{ $food }。
pet-loyal = 你的{ $pet }崇拜地看着你。
pet-growl = 你的{ $pet }向你低吼！
pet-confused = 你的{ $pet }看起来很困惑。
pet-injured = 你的{ $pet }看起来受伤了。
pet-healed = 你的{ $pet }看起来更健康了。
pet-level-up = 你的{ $pet }似乎更有经验了！
pet-died = 你的{ $pet }被杀死了！
pet-revived = 你的{ $pet }被复活了！
pet-attack-monster = 你的{ $pet }攻击了{ $monster }！
pet-fetch = 你的{ $pet }捡回了{ $item }。
pet-saddle = 你给你的{ $pet }装上了鞍。

## ============================================================================
## 饥饿 — 进食效果、尸体效果、固有属性
## ============================================================================

eat-gain-strength = 你感到力大无穷！
eat-gain-telepathy = 你感到一种奇异的心灵感应。
eat-gain-invisibility = 你感到自己变得透明了！
eat-gain-poison-resist = 你感到身体健康！
eat-gain-fire-resist = 你感到一阵凉意。
eat-gain-cold-resist = 你感到温暖。
eat-gain-sleep-resist = 你感到精神十足！
eat-gain-shock-resist = 你感到自己被绝缘了。
eat-tainted = 呃——那食物变质了！
eat-corpse-taste = 这具{ $corpse }的味道{ $taste ->
    [terrible] 糟糕透了
    [bland] 很淡
    [okay] 还行
   *[normal] 就像{ $corpse }的味道
}！
eat-petrify = 你感到自己正在变成石头！
eat-polymorph = 你感到身体正在发生变化！
eat-stun = 你晃了一下。
eat-hallucinate = 哇哦！你感到飘飘欲仙！
eat-acidic = 酸性食物灼烧了你的胃！

## ============================================================================
## 行为准则 — 违反和成就
## ============================================================================

conduct-vegetarian-break = 你打破了素食主义准则。
conduct-vegan-break = 你打破了纯素准则。
conduct-foodless-break = 你打破了不进食准则。
conduct-atheist-break = 你打破了无神论准则。
conduct-weaponless-break = 你打破了徒手准则。
conduct-pacifist-break = 你打破了和平主义准则。
conduct-illiterate-break = 你打破了文盲准则。
conduct-genocideless-break = 你打破了不灭绝准则。
conduct-polypileless-break = 你打破了不使用变化堆准则。
conduct-polyself-break = 你打破了不自我变化准则。
achievement-unlock = 成就解锁：{ $name }！
achievement-sokoban = 你解开了仓库番的谜题！
achievement-mines-end = 你到达了侏儒矿坑的底部！
achievement-medusa = 你击败了美杜莎！
achievement-castle = 你攻破了城堡！
achievement-amulet = 你获得了Yendor的护身符！

## ============================================================================
## 怪物AI — 物品使用、贪婪行为
## ============================================================================

monster-reads = { $monster }阅读了一张卷轴！
monster-uses-wand = { $monster }使用了一根{ $wand_type }魔杖！
monster-quaffs = { $monster }喝了一瓶药水！
monster-puts-on = { $monster }穿上了{ $item }。
monster-removes = { $monster }脱下了{ $item }。
monster-heals = { $monster }看起来更健康了！
monster-teleport-away = { $monster }传送走了！
monster-covetous-approach = { $monster }气势汹汹地逼近了！
monster-covetous-steal = { $monster }从你手中抢走了{ $item }！
monster-covetous-flee = { $monster }带着{ $item }撤退了！
monster-unlock = { $monster }打开了门锁。
monster-open-door = { $monster }推开了门。
monster-close-door = { $monster }关上了门。
monster-break-door = { $monster }破开了门！
monster-dig = { $monster }在墙上挖了个洞！

## ============================================================================
## 特殊层级 — 仓库番、矿坑、神谕者等
## ============================================================================

level-sokoban-enter = 你进入了一个看起来像谜题的房间。
level-sokoban-solve = 咔嗒！你听到一扇门被打开了。
level-sokoban-cheat = 你听到一阵隆隆声。
level-mines-enter = 你进入了侏儒矿坑。
level-mines-town = 你进入了矿镇。
level-oracle-enter = 你看到一个大房间，中间有一座奇特的喷泉。
level-oracle-speak = 神谕者开口了……
level-oracle-consult = 神谕者愿意以{ $price }枚金币的价格分享智慧。
level-oracle-rumor = 神谕者揭示道："{ $rumor }"
level-castle-enter = 你进入时感到一阵恐惧。
level-vlad-tower = 你感到一股冰冷的气息。
level-sanctum-enter = 你有一种奇异的不祥之感……
level-astral-enter = 你到达了星界！

## ============================================================================
## 得分和结局 — 扩展消息
## ============================================================================

score-display = 分数：{ $score }
score-rank = 你排名第{ $rank }。
score-high-new = 新的最高分！
score-high-list-title = 最高分排行榜
score-high-header = 排名  分数  条目
score-high-row = { $rank }. { $score }分  { $name }，{ $role }（{ $gender } { $race } { $alignment }），{ $cause }，位于 { $depth }
score-high-entry = { $rank }. { $role }{ $name }（{ $score }分）
score-gold-collected = 收集金币：{ $amount }
score-monsters-killed = 击杀怪物：{ $count }
score-deepest-level = 到达最深层级：{ $depth }
score-death-by = 在地牢第{ $depth }层被{ $killer }杀死。
score-escaped-with = 你以{ $score }分逃出了地牢。
score-ascended-with = 你以{ $score }分飞升了！
game-over-conduct-title = 自愿挑战：
game-over-conduct-item = 你遵守了{ $conduct }准则。
game-over-dungeon-overview = 地牢概览：
game-over-vanquished = 被消灭的生物：
game-over-genocided = 被灭绝的物种：


## ============================================================================
## 引擎国际化键 — 移动
## ============================================================================

diagonal-squeeze-blocked = 你没法从那个对角间隙挤过去。
door-no-closed = 那里没有关着的门。
door-no-open = 那里没有开着的门。
door-no-kick = 那里没有门可以踢。
pet-swap = 你和你的宠物交换了位置。
pet-nearby = 你的{ $pet }在附近。

## ============================================================================
## 引擎国际化键 — 卷轴（扩展）
## ============================================================================

scroll-identify-one = 你鉴定了一件物品。
scroll-identify-count = 你鉴定了{ $count }件物品。
scroll-nothing-to-identify = 你没有东西需要鉴定。
scroll-enchant-weapon-fragile = 你的武器感觉变脆弱了。
scroll-enchant-weapon-film = 你的武器覆盖了一层薄膜。
scroll-enchant-weapon-evaporate = 你的武器蒸发了！
scroll-enchant-weapon-vibrate = 你的武器突然剧烈震动！
scroll-enchant-armor-skin = 你的皮肤闪了一下光然后褪去了。
scroll-enchant-armor-fragile = 你的护甲感觉变脆弱了。
scroll-enchant-armor-film = 你的护甲覆盖了一层薄膜。
scroll-enchant-armor-evaporate = 你的护甲蒸发了！
scroll-enchant-armor-vibrate = 你的护甲突然剧烈震动！
scroll-remove-curse-malignant = 你感到一种邪恶的气息包围着你。
scroll-remove-curse-blessed = 你感到与万物合一。
scroll-remove-curse-punishment = 你的惩罚被解除了！
scroll-disintegrate = 卷轴碎裂了。
scroll-confuse-cursed = 你的双手抽搐了一下。
scroll-teleport-disoriented = 你感到非常迷失。
scroll-trap-detection = 你感觉到陷阱的存在。
scroll-scare-wailing = 你听到远处传来悲伤的哀嚎。
scroll-scare-dust = 你捡起卷轴时它化为了灰烬。
scroll-fire-burn = 卷轴着火了，烧伤了你的手。
scroll-earth-rocks = 石头从你周围落下！
scroll-earth-boulders = 巨石从你周围落下！
scroll-amnesia-spells = 你忘记了你的法术！
scroll-destroy-armor-itch = 你的皮肤发痒。
scroll-destroy-armor-glow = 你的护甲发出光芒。
scroll-destroy-armor-crumble = 你的护甲碎裂成灰了！
scroll-taming-growl = 你听到愤怒的咆哮！
scroll-genocide-guilty = 你感到愧疚。
scroll-genocide-prompt = 你想灭绝什么怪物？
scroll-genocide-prompt-class = 你想灭绝哪一类怪物？
scroll-light-sparkle = 微光在你周围闪烁。
scroll-charging-drained = 你感到精力被耗尽。
scroll-charging-id = 这是一张充能卷轴。
scroll-charging-nothing = 你没有可以充能的东西。
scroll-magic-mapping-fail = 不幸的是，你无法理解那些细节。
scroll-create-monster-horde = 一群怪物出现了！

## ============================================================================
## 引擎国际化键 — 陷阱（扩展）
## ============================================================================

trap-arrow-shoot = 一支箭向你射来！
trap-dart-shoot = 一支小飞镖向你射来！
trap-dart-poison-resist = 飞镖有毒，但毒药似乎对你没有效果。
trap-trapdoor-ceiling = 天花板上的活板门打开了，但什么也没有掉下来！
trap-sleeping-gas-sleep = 一团毒气让你昏睡了！
trap-fire-resist = 一根火柱从地板喷发了！但你抵抗了效果。
trap-rolling-boulder-trigger = 咔嗒！你触发了一个滚石陷阱！
trap-teleport-wrench = 你感到一阵扭曲。
trap-web-tear = 你撕裂了蛛网！
trap-web-free = 你从蛛网中挣脱了。
trap-web-stuck = 你被蛛网困住了。
trap-magic-trap-blind = 你被一道闪光弄瞎了！
trap-door-booby = 门上有诡雷！
trap-gas-puff = 一股毒气吞没了你！
trap-gas-cloud = 一团毒气包围了你！
trap-shock = 你被电击了！
trap-chest-explode = 轰！！箱子爆炸了！
trap-pit-float = 你从坑里飘了出来。
trap-bear-rip-free = 你用力挣脱了捕兽夹！
trap-cannot-disarm = 这里没有可以拆除的陷阱。
trap-disarm-fail = 你没能拆除陷阱。

## ============================================================================
## 引擎国际化键 — 传送
## ============================================================================

teleport-random = 你被传送了！
teleport-controlled = 你想传送到哪里？
teleport-invalid-target = 你无法传送到那里！
teleport-level = 你被传送到了另一层！
teleport-same-level = 你颤抖了一下。
teleport-restricted = 一股神秘的力量阻止了你传送！
teleport-branch = 你感到自己被拉到了地牢的另一个分支！
teleport-monster = 一只怪物从视野中消失了！
teleport-no-portal = 你感到一阵扭曲，但什么都没发生。
teleport-trap-controlled = 你被陷阱传送了！你有传送控制能力。
teleport-trap-restricted = 一股神秘的力量阻止了你传送！

## ============================================================================
## 引擎国际化键 — 移动（扩展）
## ============================================================================

ice-slide = 你在冰面上滑行！
ice-fumble-fall = 你在冰面上滑倒了！
water-float-over = 你漂浮在水面上。
water-swim = 你在水中游泳。
water-drown-danger = 你快要淹死了！
lava-float-over = 你漂浮在岩浆上方。
lava-resist = 岩浆灼烧着你，但你抵抗了大部分伤害。
lava-burn = 岩浆严重灼伤了你！
fumble-trip = 你被自己的脚绊倒了！

## ============================================================================
## 引擎国际化键 — 吞噬
## ============================================================================

engulf-attack-interior = 你攻击了怪物的内部！
engulf-escaped = 你从吞噬你的怪物中逃脱了！
engulf-monster-dies = 吞噬你的怪物死了！

## ============================================================================
## 引擎国际化键 — 药水（扩展）
## ============================================================================

potion-blindness-cure = 你的视力恢复了。
potion-gain-ability-str = 你感到强壮！
potion-paralysis-brief = 你短暂地僵硬了一下。
potion-no-effect = 你感到缺少了什么。
potion-sickness-deadly = 你感到病入膏肓。
potion-booze-passout = 你晕过去了。
potion-enlightenment = 你感到自我认知增强了……

## ============================================================================
## 引擎国际化键 — 饥饿（扩展）
## ============================================================================

eat-choke = 你被食物噎住了！
eat-dread = 你感到一阵恐惧。
eat-corpse-effect = 你感到吃那具尸体有一种不寻常的效果。
eat-weakened = 你感到虚弱了。
eat-greasy = 你的手指非常油腻。
eat-poison-resist = 你似乎没有受到毒素的影响。

## ============================================================================
## 引擎国际化键 — 宗教（扩展）
## ============================================================================

sacrifice-own-kind-anger = 你因为献祭同族而激怒了你的神！
sacrifice-own-kind-pleased = 你的神对你献祭同族感到满意。
sacrifice-pet-guilt = 你对献祭你的前宠物感到愧疚。
sacrifice-reduce-timeout = 你的祭品缩短了下次祈祷的等待时间。
pray-partial = 你的祈祷只被部分听到了。

## ============================================================================
## 引擎国际化键 — 神器（扩展）
## ============================================================================

artifact-invoke-heal = 你感觉好了一些。
artifact-invoke-energy = 你感到一股魔力涌入！
artifact-invoke-enlighten = 你感到自我认知增强了……
artifact-invoke-conflict = 你感到自己像一个煽动者。
artifact-invoke-invisible = 你感到自己变得相当透明了。
artifact-invoke-levitate = 你开始漂浮在空中！
artifact-invoke-untrap = 你感到擅长拆除陷阱。
artifact-invoke-charge = 你可以为一件物品充能。
artifact-invoke-teleport = 你感到一阵扭曲。
artifact-invoke-portal = 你感到空气中有一种微光。
artifact-invoke-arrows = 一阵箭雨出现了！
artifact-invoke-brandish = 你威风凛凛地挥舞着神器！
artifact-invoke-venom = 你甩出了一团毒液！
artifact-invoke-cold = 一阵寒气爆发了！
artifact-invoke-fire = 一个火球爆发了！
artifact-invoke-light = 一道致盲的光线射出！

## ============================================================================
## 引擎国际化键 — 魔杖（扩展）
## ============================================================================

wand-enlightenment = 你感到自我认知增强了。
wand-secret-door-detect = 你感觉到暗门的存在。

## ============================================================================
## 引擎国际化键 — 商店（扩展）
## ============================================================================

shop-free = 你免费得到了那个！
shop-return = { $shopkeeper }接受了退货。
shop-not-interested = { $shopkeeper }不感兴趣。
shop-angry-take = 「谢谢你，贱人！」
shop-restock = { $shopkeeper }似乎对补货很感激。
shop-no-debt = 你不欠任何东西。
shop-credit-covers = 你的信用额度支付了账单。
shop-stolen-amount = 你偷走了价值{ $amount }枚金币的商品。

## ============================================================================
## 物品命名 — BUC 状态标签
## ============================================================================

item-buc-blessed = 祝福的
item-buc-uncursed = 未诅咒的
item-buc-cursed = 被诅咒的

## ============================================================================
## 物品命名 — 侵蚀形容词
## ============================================================================

item-erosion-rusty = 生锈的
item-erosion-very-rusty = 非常锈的
item-erosion-thoroughly-rusty = 锈透的
item-erosion-corroded = 腐蚀的
item-erosion-very-corroded = 非常腐蚀的
item-erosion-thoroughly-corroded = 彻底腐蚀的
item-erosion-burnt = 烧焦的
item-erosion-very-burnt = 严重烧焦的
item-erosion-thoroughly-burnt = 彻底烧焦的
item-erosion-rotted = 腐烂的
item-erosion-very-rotted = 非常腐烂的
item-erosion-thoroughly-rotted = 彻底腐烂的
item-erosion-rustproof = 防锈的
item-erosion-fireproof = 防火的
item-erosion-corrodeproof = 防腐蚀的
item-erosion-rotproof = 防腐烂的

## ============================================================================
## 物品命名 — 类别特定基础名称模式
## ============================================================================

item-potion-identified = { $name }药水
item-potion-called = 被称为{ $called }的药水
item-potion-appearance = { $appearance }药水
item-potion-generic = 药水

item-scroll-identified = { $name }卷轴
item-scroll-called = 被称为{ $called }的卷轴
item-scroll-labeled = 标记为{ $label }的卷轴
item-scroll-appearance = { $appearance }卷轴
item-scroll-generic = 卷轴

item-wand-identified = { $name }魔杖
item-wand-called = 被称为{ $called }的魔杖
item-wand-appearance = { $appearance }魔杖
item-wand-generic = 魔杖

item-ring-identified = { $name }戒指
item-ring-called = 被称为{ $called }的戒指
item-ring-appearance = { $appearance }戒指
item-ring-generic = 戒指

item-amulet-called = 被称为{ $called }的护身符
item-amulet-appearance = { $appearance }护身符
item-amulet-generic = 护身符

item-spellbook-identified = { $name }魔法书
item-spellbook-called = 被称为{ $called }的魔法书
item-spellbook-appearance = { $appearance }魔法书
item-spellbook-generic = 魔法书

item-gem-stone = 石头
item-gem-gem = 宝石
item-gem-called-stone = 被称为{ $called }的石头
item-gem-called-gem = 被称为{ $called }的宝石
item-gem-appearance-stone = { $appearance }石头
item-gem-appearance-gem = { $appearance }宝石

item-generic-called = 被称为{ $called }的{ $base }

## ============================================================================
## 物品命名 — 连接词和后缀
## ============================================================================

item-named-suffix = 「{ $name }」

## ============================================================================
## 物品命名 — 冠词
## ============================================================================

item-article-the = 那个
item-article-your = 你的

## ============================================================================
## 物品命名 — 复数选择
## ============================================================================

item-count-name = { $count ->
   *[other] { $singular }
}

## ============================================================================
## 状态栏标签
## ============================================================================

status-satiated = 饱食
status-hungry = 饥饿
status-weak = 虚弱
status-fainting = 昏厥
status-not-hungry = {""}
status-starved = 饿死

## ============================================================================
## 界面 — 标题和标签
## ============================================================================

ui-inventory-title = 物品栏
ui-inventory-empty = 你没有携带任何东西。
ui-equipment-title = 装备
ui-equipment-empty = 你没有穿戴任何特殊装备。
ui-help-title = NetHack Babel 帮助
ui-message-history-title = 消息历史
ui-select-language = 选择语言
ui-more = --更多--
ui-save-prompt = 正在保存游戏...
ui-save-success = 游戏已保存。
ui-save-goodbye = 游戏已保存。再见！
ui-goodbye = 再见！
ui-game-over-thanks = 游戏结束。感谢游玩！
ui-unknown-command = 未知命令：'{ $key }'。按 ? 查看帮助。

## ============================================================================
## 界面 — 提示
## ============================================================================

prompt-drop = 丢弃什么？[a-zA-Z 或 ?*]
prompt-wield = 装备什么武器？[a-zA-Z 或 - 徒手]
prompt-wear = 穿戴什么？[a-zA-Z 或 ?*]
prompt-takeoff = 脱下什么？[a-zA-Z 或 ?*]
prompt-puton = 戴上什么？[a-zA-Z 或 ?*]
prompt-remove = 摘下什么？[a-zA-Z 或 ?*]
prompt-apply = 使用什么？[a-zA-Z 或 ?*]
prompt-throw-item = 投掷什么？[a-zA-Z 或 ?*]
prompt-throw-dir = 朝哪个方向？
prompt-zap-item = 挥动什么？[a-zA-Z 或 ?*]
prompt-zap-dir = 朝哪个方向？
prompt-open-dir = 朝哪个方向打开？
prompt-close-dir = 朝哪个方向关闭？
prompt-fight-dir = 朝哪个方向攻击？
prompt-pickup = 捡起什么？
prompt-dip-item = 蘸什么？[a-zA-Z]
prompt-dip-into = 蘸入什么？[a-zA-Z]

## ============================================================================
## 界面 — 物品栏分类标题
## ============================================================================

inv-class-weapon = 武器
inv-class-armor = 防具
inv-class-ring = 戒指
inv-class-amulet = 护身符
inv-class-tool = 工具
inv-class-food = 食物
inv-class-potion = 药水
inv-class-scroll = 卷轴
inv-class-spellbook = 魔法书
inv-class-wand = 魔杖
inv-class-coin = 金币
inv-class-gem = 宝石
inv-class-rock = 石头
inv-class-ball = 铁球
inv-class-chain = 铁链
inv-class-venom = 毒液
inv-class-other = 其他

# 物品栏 BUC 标记
inv-buc-marker-blessed = [祝]
inv-buc-marker-cursed = [咒]
inv-buc-tag-blessed = （祝福）
inv-buc-tag-cursed = （诅咒）
inv-buc-tag-uncursed = （未诅咒）
ui-pickup-title = 拾取什么？

## ============================================================================
## 事件消息
## ============================================================================

event-hp-gained = 你感觉好多了。
event-hp-lost = 哎哟！
event-pw-gained = 你感到魔力回涌。
event-you-see-here = 你看到这里有{ $terrain }。
event-dungeon-welcome = 你发现自己身处一座地牢中。祝你好运！
event-player-role = 你是{ $align }{ $race }{ $role }{ $name }。

## ============================================================================
## 地形名称
## ============================================================================

terrain-floor = 地板
terrain-corridor = 走廊
terrain-stone = 实心岩壁
terrain-wall = 墙壁
terrain-closed-door = 关闭的门
terrain-open-door = 打开的门
terrain-locked-door = 锁住的门
terrain-stairs-up = 向上的楼梯
terrain-stairs-down = 向下的楼梯
terrain-fountain = 喷泉
terrain-altar = 祭坛
terrain-throne = 王座
terrain-sink = 水槽
terrain-grave = 坟墓
terrain-pool = 水池
terrain-moat = 护城河
terrain-ice-terrain = 冰面
terrain-air = 空气
terrain-cloud = 云层
terrain-water = 水
terrain-lava = 岩浆
terrain-trap = 陷阱
terrain-tree = 树
terrain-iron-bars = 铁栅栏
terrain-drawbridge = 吊桥
terrain-magic-portal = 魔法传送门

## ============================================================================
## 引擎 — 陷阱消息
## ============================================================================

trap-shiver = 你突然打了个寒颤。
trap-howl = 你听到远处的嚎叫声。
trap-yearning = 你感到一阵奇怪的渴望。
trap-pack-shakes = 你的背包剧烈摇晃！
trap-fumes = 你闻到刺鼻的烟雾。
trap-tired = 你突然感到很累。

## ============================================================================
## 引擎 — 鉴定类别名称
## ============================================================================

id-class-potion = 药水
id-class-scroll = 卷轴
id-class-ring = 戒指
id-class-wand = 魔杖
id-class-spellbook = 魔法书
id-class-amulet = 护身符
id-class-weapon = 武器
id-class-armor = 防具
id-class-tool = 工具
id-class-food = 食物
id-class-coin = 金币
id-class-gem = 宝石
id-class-rock = 石头
id-class-ball = 铁球
id-class-chain = 铁链
id-class-venom = 毒液
id-class-unknown = 东西
id-unknown-object = 奇怪的物体
id-something = 某物

## ============================================================================
## 引擎 — 商店类型名称
## ============================================================================

shop-type-general = 杂货店
shop-type-armor = 二手铠甲店
shop-type-book = 二手书店
shop-type-liquor = 酒庄
shop-type-weapon = 古董武器店
shop-type-deli = 熟食店
shop-type-jewel = 珠宝店
shop-type-apparel = 高级服饰店
shop-type-hardware = 五金店
shop-type-rare-book = 珍本书店
shop-type-health = 保健食品店
shop-type-lighting = 灯具店

## ============================================================================
## 引擎 — 宠物种类名称
## ============================================================================

pet-little-dog = 小狗
pet-kitten = 小猫
pet-pony = 小马

## ============================================================================
## 引擎 — 阵营名称
## ============================================================================

align-law = 秩序
align-balance = 中立
align-chaos = 混沌

## ============================================================================
## BUC 标记（物品栏显示）
## ============================================================================

buc-tag-blessed = （祝福）
buc-tag-cursed = （诅咒）
buc-tag-uncursed = （未诅咒）
buc-marker-blessed = [祝]
buc-marker-cursed = [咒]

## ============================================================================
## 角色创建 — 职业
## ============================================================================

role-archeologist = 考古学家
role-barbarian = 野蛮人
role-caveperson = 穴居人
role-healer = 治愈者
role-knight = 骑士
role-monk = 武僧
role-priest = 牧师
role-ranger = 游侠
role-rogue = 盗贼
role-samurai = 武士
role-tourist = 旅行者
role-valkyrie = 女武神
role-wizard = 巫师

## ============================================================================
## 角色创建 — 种族
## ============================================================================

race-human = 人类
race-elf = 精灵
race-dwarf = 矮人
race-gnome = 侏儒
race-orc = 兽人

## ============================================================================
## 角色创建 — 阵营
## ============================================================================

alignment-lawful = 守序
alignment-neutral = 中立
alignment-chaotic = 混沌

## ============================================================================
## 角色创建 — 提示
## ============================================================================

chargen-pick-role = 选择职业：
chargen-pick-race = 选择种族：
chargen-pick-alignment = 选择阵营：
chargen-who-are-you = 你叫什么名字？[默认：{ $default }]

## ============================================================================
## 状态栏标签 — 第一行（属性）
## ============================================================================

stat-label-str = 力
stat-label-dex = 敏
stat-label-con = 体
stat-label-int = 智
stat-label-wis = 感
stat-label-cha = 魅

## ============================================================================
## 状态栏标签 — 第二行（地下城状态）
## ============================================================================

stat-label-dlvl = 深度
stat-label-gold = $
stat-label-hp = 生命
stat-label-pw = 魔力
stat-label-ac = 防御
stat-label-xp = 经验
stat-label-turn = 轮
stat-status-blind = 盲
stat-status-conf = 乱
stat-status-stun = 晕
stat-status-hallu = 幻
stat-status-lev = 浮
stat-status-ill = 病
stat-enc-burdened = 负重
stat-enc-stressed = 重负
stat-enc-strained = 吃力
stat-enc-overtaxed = 超载
stat-enc-overloaded = 过载
stat-branch-mines = 矿坑
stat-branch-sokoban = 推箱
stat-branch-quest = 任务
stat-branch-gehennom = 地狱
stat-branch-vlad = 弗拉德
stat-branch-knox = 诺克斯
stat-branch-earth = 地
stat-branch-air = 风
stat-branch-fire = 火
stat-branch-water = 水
stat-branch-astral = 星界
stat-branch-end = 终

## ============================================================================
## 设置菜单
## ============================================================================

ui-options-title = 设置
ui-options-game = 游戏设置
ui-options-display = 显示设置
ui-options-sound = 音效设置

## 游戏选项

opt-autopickup = 自动拾取
opt-autopickup-types = 自动拾取类型
opt-legacy = 开场叙事

## 显示选项

opt-map-colors = 地图颜色
opt-message-colors = 消息颜色
opt-buc-highlight = 祝福/诅咒高亮
opt-minimap = 小地图
opt-mouse-hover = 鼠标悬停信息
opt-nerd-fonts = Nerd 字体

## 音效选项

opt-sound-enabled = 音效
opt-volume = 音量

## 选项值

opt-on = 开
opt-off = 关
ui-options-game-title = 游戏设置（共 { $count } 项）
ui-options-edit-prompt = { $option } [{ $current }]：
ui-oracle-menu-title = 咨询神谕者
ui-oracle-minor-option = 小型咨询（50 金币）
ui-oracle-major-option = 大型咨询（{ $amount } 金币）
ui-cancel = 取消
ui-choice-prompt = 选择>
ui-loot-menu-title = 搜刮 { $container }
ui-loot-option-take-out = 取出：{ $item }
ui-loot-option-take-all = 全部取出
ui-loot-option-put-in = 放入：{ $item }
ui-loot-option-put-all = 全部放入
ui-demon-bribe-prompt = 你要出价多少？[0..{ $amount }]（留空表示拒绝，Esc 取消）
ui-demon-bribe-text-title = 你要出价多少？[0..{ $amount }]
ui-demon-bribe-help = 留空或非法输入都会视为拒绝
ui-offer-prompt = 出价>
ui-confirm-quit = 真的要退出吗？
ui-save-failed = 保存失败：{ $error }
ui-save-create-dir-failed = 创建存档目录失败：{ $error }
ui-save-load-warning = 警告：读取存档失败：{ $error }
ui-save-load-new-game = 将开始新游戏。
ui-commands-title = 命令
ui-here-commands-title = 此处相关命令
ui-there-commands-title = 目标位置相关命令
ui-direction-prompt = 往哪个方向？
ui-direction-prompt-optional = 往哪个方向？（Esc 取消）
ui-direction-prompt-run = 朝哪个方向奔跑？
ui-direction-prompt-rush = 朝哪个方向猛冲？
ui-direction-help-title = 方向键帮助
ui-direction-help-body = h 向左，j 向下，k 向上，l 向右；y 左上，u 右上，b 左下，n 右下；. 原地，< 上楼，> 下楼；Esc 取消，? 显示此帮助
ui-direction-invalid = 这不是方向输入。按 ? 查看帮助。
ui-no-previous-command = 没有可重复的上一条命令。
ui-press-any-key-continue = （按任意键继续）
ui-count-prefix = 计数
ui-recording-start = 正在录制会话到：{ $path }
ui-recording-saved = 会话已录制到：{ $path }
ui-recording-save-warning = 警告：保存录制失败：{ $error }
ui-options-volume-prompt = { $option }（0-100）：
ui-text-commands-summary = 命令：h/j/k/l/y/u/b/n 移动，. 原地等待，s 搜索，, 拾取，i 背包，eq 装备，p 祈祷，< 上楼，> 下楼，q 退出，? 帮助
ui-text-status-line = 深度:{ $depth }  { $hp }  轮:{ $turn }  位置:{ $pos }  [hjklyubn=移动 .=等待 <=上楼 >=下楼 q=退出 ?=帮助]
ui-startup-loaded-filesystem = 已从 { $path } 加载 { $monsters } 种怪物、{ $objects } 种物品
ui-startup-loaded-embedded = 已从内嵌资源加载 { $monsters } 种怪物、{ $objects } 种物品
ui-startup-language = 当前语言：{ $code }（{ $name }）
ui-restored-save = 已恢复存档：回合 { $turn }，深度 { $depth }。

option-label-autopickup = 自动拾取
option-label-autodig = 自动挖掘
option-label-autoopen = 自动开门
option-label-autoquiver = 自动准备投射物
option-label-cmdassist = 命令辅助
option-label-confirm = 危险操作确认
option-label-extmenu = 扩展命令菜单
option-label-fireassist = 射击辅助
option-label-fixinv = 固定物品栏字母
option-label-force-invmenu = 强制使用物品菜单
option-label-lootabc = 战利品字母菜单
option-label-number-pad = 数字键盘移动
option-label-pickup-stolen = 允许拾取赃物
option-label-pickup-thrown = 自动捡回投掷物
option-label-pushweapon = 压入上一把武器
option-label-quick-farsight = 快速远望
option-label-rest-on-space = 空格休息
option-label-safe-pet = 保护宠物
option-label-safe-wait = 安全等待
option-label-sortpack = 整理背包
option-label-travel = 自动寻路
option-label-verbose = 详细消息
option-label-autopickup-types = 自动拾取类别
option-label-menustyle = 菜单样式
option-label-pile-limit = 物品堆上限
option-label-runmode = 连续移动模式
option-label-sortloot = 战利品排序
option-label-color = 彩色显示
option-label-dark-room = 黑暗房间渲染
option-label-hilite-pet = 高亮宠物
option-label-hilite-pile = 高亮物品堆
option-label-lit-corridor = 点亮走廊
option-label-sparkle = 闪烁特效
option-label-standout = 突出显示
option-label-use-inverse = 反色强调
option-label-hitpointbar = 生命条
option-label-showexp = 显示经验
option-label-showrace = 显示种族
option-label-showscore = 显示分数
option-label-time = 显示时间
option-label-fruit = 自定义水果名
option-label-name = 角色名称
option-label-packorder = 背包顺序
option-label-tombstone = 墓碑画面
option-label-mail = 邮件通知

option-value-traditional = 传统
option-value-combination = 组合
option-value-full = 完整
option-value-partial = 简略
option-value-teleport = 传送
option-value-run = 奔跑
option-value-walk = 行走
option-value-crawl = 爬行
option-value-none = 无
option-value-loot = 仅战利品

## ============================================================================
## 传承序言
## ============================================================================

legacy-intro =
    《{ $deity }之书》中写道：

        创世之后，残忍的神莫洛克叛变了
        创造者马杜克的权威。
        莫洛克从马杜克手中偷走了众神最强大的
        神器——耶恩德的护身符，
        将其藏匿在黑暗的深渊之中——
        冥府格亨诺姆，他在那里潜伏至今，
        等待时机。

    你的神{ $deity }渴望拥有护身符，
    并借此在众神之上获得应有的至高地位。

    你，一名初出茅庐的{ $role }，
    从出生起就被预言为{ $deity }的使者。
    你注定要为你的神找回护身符，
    或在尝试中死去。你命运的时刻已经到来。
    为了我们所有人：勇敢地与{ $deity }同行！

## ============================================================================
## TUI 常用消息
## ============================================================================

ui-never-mind = 没关系。
ui-no-such-item = 你没有那个物品。
ui-not-implemented = 尚未实现。
ui-empty-handed = 你空手着。

## ============================================================================
## 动作分派
## ============================================================================

eat-generic = 你吃了食物。
eat-what = 吃什么？
quaff-generic = 你喝了药水。
quaff-what = 喝什么？
read-generic = 你读了卷轴。
read-what = 读什么？
zap-generic = 你使用了魔杖。

## 门
door-open-success = 门开了。
door-already-open = 这扇门已经开了。
door-not-here = 那里没有门。
door-close-success = 门关上了。
door-already-closed = 这扇门已经关了。

## 锁
lock-nothing-to-force = 这里没有可以强行打开的东西。

## 祈祷
pray-begin = 你开始向神灵祈祷……

## 祭品
offer-generic = 你在祭坛上献上了祭品。
offer-amulet-rejected = 护符被拒绝了，并落在你附近！
offer-what = 献上什么？

## 对话
npc-chat-no-response = 这个生物似乎不想聊天。
npc-chat-sleeping = 这个生物似乎根本没注意到你。
npc-chat-deaf-response = 对方就算回应了，你也听不见。
chat-nobody-there = 那里没有人可以交谈。
chat-up = 上面的人听不见你说话。
chat-down = 下面的人听不见你说话。
chat-self = 自言自语对地牢冒险者可不是什么好习惯。
chat-cannot-speak = 以 { $form } 的形态，你无法说话。
chat-strangled = 你说不出话来。你快窒息了！
chat-swallowed = 外面的人听不见你说话。
chat-underwater = 在水下，你的话谁也听不清。
chat-statue = 雕像似乎根本没注意到你。
chat-wall = 这简直就像在对着墙说话。
chat-wall-hallu-gripes = 墙壁开始抱怨自己的差事。
chat-wall-hallu-joke = 墙壁给你讲了个很好笑的笑话！
chat-wall-hallu-insults = 墙壁狠狠辱骂了你的出身！
chat-wall-hallu-uninterested = 墙壁看起来对你毫无兴趣。

## 移动/旅行
peaceful-monster-blocks = 你停了下来。{ $monster } 挡住了去路。
ride-not-available = 这里没有可以骑乘的东西。
enhance-not-available = 你现在无法提升任何技能。
enhance-success = 你的 { $skill } 提升到了 { $level }。
travel-not-implemented = 旅行功能尚未实现。
two-weapon-not-implemented = 双武器战斗尚未实现。
two-weapon-enabled = 你开始双武器战斗。
two-weapon-disabled = 你停止双武器战斗。
name-not-implemented = 命名功能尚未实现。
adjust-not-implemented = 物品调整功能尚未实现。

## ============================================================================
## 任务/NPC 对话
## ============================================================================

quest-leader-greeting = 欢迎，{ $role }。我一直在等你。
quest-assignment =
    听好了，{ $role }。{ $nemesis }偷走了{ $artifact }。
    你必须深入地下找回它。
    我们的命运全靠你了。

## 店主
shop-welcome = 欢迎来到{ $shopkeeper }的{ $shoptype }！
shop-buy-prompt = { $shopkeeper }说："现金还是赊账？"
shop-unpaid-warning = { $shopkeeper }说："你有未付款的物品！"
shop-theft-warning = { $shopkeeper }大喊："小偷！快付钱！"

## 祭司
priest-welcome = 祭司向你念诵了祝福。
priest-protection-offer = 祭司提供神圣保护，需要{ $cost }金币。
priest-donation-thanks = 祭司感谢你慷慨的捐赠。
priest-ale-gift = 祭司给了你 { $amount } 枚金币买酒。
priest-cheapskate = 祭司怀疑地看着你寒酸的捐赠。
priest-small-thanks = 祭司感谢你尽力拿出的这点捐赠。
priest-pious = 祭司说你确实相当虔诚。
priest-clairvoyance = 祭司赐予你片刻的洞察。
status-clairvoyance-end = 你的洞察渐渐消退。
priest-selfless-generosity = 祭司深深感激你无私的慷慨。
priest-cleansing = 祭司的祝福减轻了你的精神负担。
priest-cranky-1 = 祭司厉声道：“你还想说话？那我就跟你说道说道！”
priest-cranky-2 = 祭司冷声道：“想聊天？这就是我要说的话！”
priest-cranky-3 = 祭司说道：“朝圣者，我已不想再同你多言。”

## ============================================================================
## 内容传递（谣言、神谕）
## ============================================================================

rumor-fortune-cookie = 你打开了幸运饼干，上面写着："{ $rumor }"
oracle-consultation = { $text }
oracle-no-mood = 神谕者现在没有心情接受咨询。
oracle-no-gold = 你身上一枚金币也没有。
oracle-not-enough-gold = 你连这个价钱都付不起！

## ============================================================================
## 符号识别
## ============================================================================

whatis-prompt = 你想识别什么？（选择一个位置）
whatis-terrain = { $description }（地形）
whatis-monster = { $description }（怪物）
whatis-object = { $description }（物品）
whatis-nothing = 你没有看到什么特别的东西。

## 发现
discoveries-title = 已发现
discoveries-empty = 你还没有发现任何东西。

## ═══ Untranslated (English fallback) ═══

already-mounted = 你已经骑在坐骑上了。

already-punished = 你已经在受罚中了。

attack-acid-hit = 酸液溅了你一身！

attack-acid-resisted = 酸液似乎伤不到你。

attack-breath = { $monster } 朝你喷吐！

attack-cold-hit = 你浑身覆满寒霜！

attack-cold-resisted = 你只是觉得微微发冷。

attack-disease = 你觉得病得厉害。

attack-disintegrate = 你被解离了！

attack-disintegrate-resisted = 你没有被解离。

attack-drain-level = 你感到生命力正从体内流失！

attack-engulf = { $monster } 把你吞没了！

attack-fire-hit = 火焰吞没了你！

attack-fire-resisted = 你只是觉得微微发热。

attack-hug-crush = 你正被越勒越紧！

attack-paralyze = 你僵在原地，动弹不得！

attack-poisoned = 你觉得恶心得厉害！

attack-shock-hit = 电流猛地击中了你！

attack-shock-resisted = 你只是感到一阵轻微麻刺。

attack-sleep = 你感到昏昏欲睡……

attack-slowed = 你觉得自己动作变慢了。

attack-stoning-start = 你开始变成石头了！

boulder-blocked = 这块巨石被卡住了。

boulder-fills-pit = 巨石填平了陷坑！

boulder-push = 你推动了巨石。

call-empty-name = 你没有取名字。

cannot-do-that = 你不能那么做。

choke-blood-trouble = 你感到呼吸困难。

choke-consciousness-fading = 你的意识正在消退……

choke-gasping-for-air = 你正大口喘着气！

choke-hard-to-breathe = 你感到呼吸困难。

choke-neck-constricted = 你的脖子被勒住了！

choke-neck-pressure = 你感到脖子上有股压力。

choke-no-longer-breathe = 你再也无法呼吸了。

choke-suffocate = 你窒息而亡。

choke-turning-blue = 你的脸都发青了。

chronicle-empty = 你的编年史是空的。

clairvoyance-nothing-new = 你没有感知到什么新东西。

container-put-in = 你把 { $item } 放进了 { $container }。

container-take-out = 你从 { $container } 中取出了 { $item }。

crystal-ball-cloudy = 你只看到一团翻腾旋转的混沌。

crystal-ball-nothing-new = 你没有看到什么新东西。

cursed-cannot-remove = 你拿不下来，它被诅咒了！

detect-food-none = 你没有感知到任何食物。

detect-gold-none = 你没有感知到任何黄金。

detect-monsters-none = 你没有感知到任何怪物。

detect-objects-none = 你没有感知到任何物品。

detect-traps-none = 你没有感知到任何陷阱。

dig-blocked = 这里太硬了，挖不动。

dig-floor-blocked = 这里的地板太硬，挖不动。

dig-floor-hole = 你在地板上挖出了一个洞！

dig-ray-nothing = 挖掘射线没有效果。

dig-wall-done = 你挖穿了这面墙。

dip-acid-nothing = 什么也没有发生。

dip-acid-repair = 你的 { $item } 看起来完好如新！

dip-amethyst-cure = 你感到不那么困惑了。

dip-diluted = 你的 { $item } 被稀释了。

dip-excalibur = 当你把剑浸入其中时，一道奇异的光芒掠过剑身！你的剑现在名为 Excalibur！

dip-fountain-cursed = 水面短暂地发出微光。

dip-fountain-nothing = 看起来什么也没有发生。

dip-fountain-rust = 你的 { $item } 生锈了！

dip-holy-water = 你把 { $item } 浸入了圣水中。

dip-no-fountain = 这里没有可供浸泡的喷泉。

dip-not-a-potion = 那不是药水！

dip-nothing-happens = 看起来什么也没有发生。

dip-poison-weapon = 你给 { $item } 涂上了毒药。

dip-unholy-water = 你把 { $item } 浸入了不洁之水中。

dip-unicorn-horn-cure = 你感觉好多了。

djinni-from-bottle = 一个巨大的灯神从瓶中现身！

drawbridge-destroyed = 吊桥被毁掉了！

drawbridge-lowers = 吊桥放下来了！

drawbridge-raises = 吊桥升起来了！

drawbridge-resists = 吊桥没有反应！

end-ascension-offering = 你向 { $god } 献上了 Yendor 护符……

end-do-not-pass-go = 不要经过起点，也不要领取 200 佐克币。

engrave-elbereth = 你在地上刻下了「Elbereth」。

engulf-ejected = 你被 { $monster } 吐了出来！

engulf-escape-killed = 你在 { $monster } 体内将它杀死了！

fire-no-ammo = 你没有合适的东西可射击。

fountain-chill = 你感到一阵寒意。

fountain-curse-items = 你顿时有种失落感。

fountain-dip-curse = 水面闪了一下光。

fountain-dip-nothing = 看起来什么也没有发生。

fountain-dip-uncurse = 水面闪了一下光。

fountain-dried-up = 喷泉已经干涸了！

fountain-dries-up = 喷泉干涸了！

fountain-find-gem = 你感到这里有颗宝石！

fountain-foul = 水变脏了！你干呕并吐了出来。

fountain-gush = 水从满溢的喷泉里涌了出来！

fountain-no-position = 你不能从这个位置去浸泡。

fountain-not-here = 这里没有喷泉。

fountain-nothing = 一颗大气泡冒上水面后破掉了。

fountain-poison = 水被污染了！

fountain-refresh = 清凉的气息让你精神一振。

fountain-see-invisible = 你感到自己有了自知之明……

fountain-see-monsters = 你感到邪恶的存在。

fountain-self-knowledge = 你感到自己有了自知之明……

fountain-shimmer = 你看见一汪闪烁的水池。

fountain-tingling = 一阵奇异的刺麻感沿著你的手臂窜上来。

fountain-water-demon = 无尽的蛇群从里面倾泻而出！

fountain-water-moccasin = 无尽的蛇群从里面倾泻而出！

fountain-water-nymph = 一缕薄雾从喷泉中逸散出来……

ghost-from-bottle = 当你打开瓶子时，里面冒出了什么东西。

god-lightning-bolt = 突然，一道闪电击中了你！

grave-corpse = 你在坟墓里发现了一具尸体。

grave-empty = 这座坟墓里空无一物。真奇怪……

guard-halt = “站住，小偷！你被捕了！”

guard-no-gold = 卫兵没在你身上搜到金币。

guardian-angel-appears = 一位守护天使出现在你身旁！
guardian-angel-rebukes = 你的守护天使斥责了你！

hunger-faint = 你因缺乏食物而昏倒了。

hunger-starvation = 你因饥饿而死。

instrument-no-charges = 这件乐器已经没有充能了。

intrinsic-acid-res-temp = 你感到一阵短暂的刺痛。

intrinsic-cold-res = 你感觉自己满肚子热气。

intrinsic-disint-res = 你觉得自己非常坚实。

intrinsic-fire-res = 你感到一阵短暂的寒意。

intrinsic-invisibility = 你觉得自己轻飘飘的。

intrinsic-poison-res = 你感觉很健康。

intrinsic-see-invisible = 你觉得自己洞察敏锐！

intrinsic-shock-res = 你感觉自己的生命力被放大了！

intrinsic-sleep-res = 你觉得自己精神抖擞。

intrinsic-stone-res-temp = 你觉得自己格外灵活。

intrinsic-strength = 你觉得自己力大无穷！

intrinsic-telepathy = 你感到精神异常敏锐。

intrinsic-teleport-control = 你感到自己能掌控自身。

intrinsic-teleportitis = 你感到自己很焦躁不安。

invoke-no-power = 看起来什么也没有发生。

invoke-not-wielded = 你必须持在手上才能唤起它的力量。

jump-no-ability = 你不知道怎么跳跃。

jump-out-of-range = 那个地方太远了！

jump-success = 你跳了起来！

jump-too-burdened = 你负重太大，跳不起来！

kick-door-held = 门被顶住了！

kick-door-open = 门被你一脚踹开了！

kick-hurt-foot = 哎哟！好疼！

kick-item-blocked = 有东西挡住了你的踢击。

kick-item-moved = 你踢到了什么东西。

kick-nothing = 你朝空处踢了一脚。

kick-sink-ring = 水槽里有什么东西叮当作响。

known-nothing = 你现在还什么都不知道。

levitating-cant-go-down = 你漂浮在地板上方很高的地方。

levitating-cant-pickup = 你够不到地面。

levitation-float-lower = 你轻轻飘回到了地面上。

levitation-wobble = 你在半空中摇摇晃晃。

light-extinguished = 你的 { $item } 熄灭了。

light-lit = 你的 { $item } 现在点亮了。

light-no-fuel = 你的 { $item } 没有燃料了。

light-not-a-source = 那不是光源。

lizard-cures-confusion = 你感到不那么困惑了。

lizard-cures-stoning = 你感觉身体灵活多了！

lock-already-locked = 它已经锁上了。

lock-door-locked = 门锁上了。

lock-force-container-success = 你强行撬开了锁！

lock-force-fail = 你没能强行打开锁。

lock-force-success = 你强行撬开了锁！

lock-lockpick-breaks = 你的开锁器断掉了！

lock-need-key = 你需要一把钥匙来锁上它。

lock-no-door = 这里没有门。

lock-no-target = 这里没有可供上锁或开锁的东西。

lock-pick-container-success = 你成功撬开了锁。

lock-pick-fail = 你没能撬开这把锁。

lock-pick-success = 你成功撬开了锁。

lycanthropy-cured = 你感到自己被净化了。

lycanthropy-full-moon-transform = 今晚你觉得浑身发热。

lycanthropy-infected = 你觉得浑身发热。

magic-mapping-nothing-new = 你已经清楚周围的环境了。

mhitm-passive-stoning = { $monster } 变成了石头！

monster-ability-used = { $monster } 使用了特殊能力！

monster-no-ability = 你目前的形态没有那种能力。

monster-not-polymorphed = 你没有发生变形。

monster-scared-elbereth = { $monster } 被 Elbereth 雕刻吓住了！

monster-teleport-near = { $monster } 凭空出现了！

mount-not-monster = 那不是你能骑乘的生物。

mount-not-tame = 那只生物还不够温顺，不能骑。

mount-too-far = 那只生物离得太远了。

mount-too-weak = 你太虚弱了，骑不上去。

no-fountain-here = 这里没有喷泉。

not-a-drawbridge = 那不是吊桥。

not-a-raised-drawbridge = 那座吊桥没有升起。

not-carrying-anything = 你什么都没带。

not-mounted = 你没有骑着任何东西。

not-punished = 你没有受罚。

not-wearing-that = 你没有穿戴那个。

phaze-feeling-bloated = 你感到腹胀。

phaze-feeling-flabby = 你觉得自己软趴趴的。

play-bugle = 你吹响了军号。

play-drum = 你敲响了鼓。

play-earthquake = 整座地城都在你周围摇晃！

play-horn-noise = 你吹出了一声骇人而难听的声音。

play-magic-flute = 你奏出极其悦耳的乐音。

play-magic-harp = 你奏出极其悦耳的乐音。

play-music = 你演奏了一段乐曲。

play-nothing = 你想不出有什么合适的东西能演奏。

polymorph-controlled = 你想变成哪种怪物？
polymorph-dismount = 你不能再骑乘你的坐骑了。

polymorph-newman-survive = 你挺过了这次变形尝试。

polymorph-revert = 你恢复成原来的形态。

polymorph-system-shock = 你的身体颤抖著，经历了剧烈变形！

polymorph-system-shock-fatal = 变形造成的系统冲击杀死了你！

potion-acid-resist = 你对酸的抗性消失了！

potion-see-invisible-cursed = 你刚才好像看见了什么。

potion-sickness-mild = 呸！这东西尝起来像毒药。

potion-uneasy = 你感到有些不安。

pray-angry-curse = 你感到身上的物品好像没那么有效了。

pray-angry-displeased = 你感到 { $god } 很不高兴。

pray-angry-lose-wis = 你的智慧下降了。

pray-angry-punished = 你因为举止不当而受到了惩罚！

pray-angry-summon = { $god } summons hostile monsters!

pray-angry-zap = 突然，一道闪电劈中了你！

pray-bless-weapon = 你的武器柔和地发出光芒。

pray-castle-tune = 你听见一个声音回荡著：「密道口令听起来像……」

pray-cross-altar-penalty = 你有种奇怪的禁忌感。

pray-demon-rejected = { $god } is not deterred...

pray-fix-trouble = { $god } fixes your trouble.

pray-gehennom-no-help = { $god } does not seem to be able to reach you in Gehennom.

pray-golden-glow = 一道金色光辉笼罩了你。

pray-grant-intrinsic = 你感受到 { $god } 的力量。

pray-grant-spell = 神圣知识充满了你的脑海！

pray-indifferent = { $god } seems indifferent.

pray-moloch-laughter = 摩洛克嘲笑你的祈祷。

pray-pleased = 你感到 { $god } 很满意。

pray-uncurse-all = 你感觉好像有人在帮你。

pray-undead-rebuke = 你感到自己不配。

priest-angry = 祭司发怒了！

priest-calmed = 祭司冷静下来了。

priest-virtues-of-poverty = 祭司宣讲清贫的美德。

priest-wrong-alignment = 祭司不悦地咕哝著。

punishment-applied = 你受到了惩罚！

punishment-removed = 你感到铁球消失了。

quest-assigned = 你的任务已经指派给你了。
quest-completed = 你的任务已经完成。
quest-leader-first = { $leader } 向你致意，并衡量你的资格。
quest-leader-next = { $leader } 再次审视你，判断你是否已经准备好。
quest-leader-assigned = { $leader } 提醒你去击败 { $nemesis }。
quest-leader-nemesis-dead = { $leader } 认可你带着 { $artifact } 归来。
quest-leader-reject = { $leader } 以“{ $reason }”为由拒绝了你。
quest-guardian = { $guardian } 警告你要忠于自己的任务。
quest-nemesis-first = { $nemesis } 挡住了你的去路。
quest-nemesis-next = { $nemesis } 仍在等待你的到来。
quest-nemesis-artifact = { $nemesis } 看到任务神器时发出怒吼。
quest-nemesis-dead = 空气中弥漫着 { $nemesis } 败亡后的腐臭。
quest-expelled = 你还没有获准深入任务地城。
invocation-complete = 祈唤仪式成功了，一道魔法传送门开启了！
invocation-incomplete = 符文闪烁着，但祈唤并未完成。
invocation-missing-bell = 没有开门铃，仪式立刻失去了关键一环。
invocation-missing-candelabrum = 没有祈唤烛台，仪式无法成形。
invocation-needs-bell-rung = 必须先在这里敲响开门铃，仪式才能开始。
invocation-needs-candelabrum-ready = 祈唤烛台必须点燃七支蜡烛才行。
invocation-items-cursed = 被诅咒的祈唤道具扭曲了整个仪式。
read-dead-book = 《死者之书》低语着墓穴般的力量。

region-fog-obscures = 一团雾气遮蔽了你的视线！

reveal-monsters-none = 没有可显现的怪物。

rub-lamp-djinni = 你擦了擦灯，灯神冒了出来！

rub-lamp-nothing = 什么也没有发生。

rub-no-effect = 看起来什么也没有发生。

rub-touchstone = 你在试金石上摩擦。

rump-gets-wet = 你的屁股湿了。

sacrifice-alignment-convert = 你感到 { $god } 的力量加诸于你身上。

sacrifice-altar-convert = 祭坛被转化了！

sacrifice-altar-reject = { $god } rejects your sacrifice!

sacrifice-conversion-rejected = 你听见一声雷鸣！

sacrifice-nothing = 你的祭品消失了！

sacrifice-unicorn-insult = { $god } 认为你的祭品是在冒犯祂。

sanctum-be-gone = "滚开，凡人！"

sanctum-desecrate = 你亵渎了至高祭坛！

sanctum-infidel = "你竟敢闯入圣所，异教徒！"

scroll-confuse-cure = 你觉得自己不那么混乱了。

scroll-confuse-self = 你感到一阵混乱。

scroll-destroy-armor-disenchant = 你的护甲变得没那么有效了！

scroll-destroy-armor-id = 你的护甲闪了一下光，然后黯淡下去。

scroll-fire-confused = 你的卷轴猛然燃烧起来！

scroll-genocide-reverse = 你制造出了一大群怪物！

scroll-genocide-reverse-self = 你感觉某种变化正降临到自己身上。

scroll-identify-self = 你觉得自己对自己了如指掌……

see-item-here = 你看见这里有一件物品。
see-items-here = 你看见这里有好几件物品。

scroll-cant-read-blind = 你失明时无法阅读！

sick-deaths-door = 你已经到了死门关前。

sick-illness-severe = 你觉得病得快死了。

sit-already-riding = 你已经骑著什么东西了。

sit-in-water = 你坐进了水里。

sit-no-seats = 这里没有可以坐的东西。

sit-on-air = 坐在空气上很好玩吗？

sit-on-altar = 你坐在了祭坛上。

sit-on-floor = 坐在地板上很好玩吗？

sit-on-grave = 你坐在了墓碑上。

sit-on-ice = 冰面摸起来很冷。

sit-on-lava = 岩浆烫伤了你！

sit-on-sink = 你坐在了水槽上。

sit-on-stairs = 你坐在了楼梯上。

sit-on-throne = 你感到一阵奇怪的感觉。

sit-tumble-in-place = 你在原地打了个滚。

sleepy-yawn = 你打了个哈欠。

slime-burned-away = 黏液被烧掉了！

sliming-become-slime = 你已经变成绿色史莱姆了！

sliming-limbs-oozy = 你的四肢开始变得黏糊糊的。

sliming-skin-peeling = 你的皮肤开始剥落。

sliming-turning-green = 你开始有点发绿了。

sliming-turning-into = 你正在变成绿色史莱姆！

spell-aggravation = 你觉得好像有什么东西非常愤怒。

spell-book-full = 你已经学会太多法术了。

spell-cancellation-hit = { $target } is covered by a shimmering light!

spell-cancellation-miss = 你没打中 { $target }。

spell-cast-fail = 你没能正确施放法术。

spell-cause-fear-none = 没有怪物感到恐惧。

spell-charm-monster-hit = { $target } is charmed!

spell-charm-monster-miss = { $target } resists!

spell-clairvoyance = 你感知到周围的环境。

spell-confuse-monster-hit = { $target } seems confused!

spell-confuse-monster-miss = { $target } resists!

spell-confuse-monster-touch = 你的双手开始发红光。

spell-create-familiar = 一只熟悉的生物出现了！

spell-create-monster = 一只怪物出现了！

spell-cure-blindness-not-blind = 你没有失明。

spell-cure-sickness-not-sick = 你没有生病。

spell-curse-items = 你感到自己好像需要驱魔师。

spell-destroy-armor = 你的护甲崩解了！

spell-detect-monsters-none = 你没有感知到任何怪物。

spell-detect-unseen-none = 你没有感知到任何隐形之物。

spell-dig-nothing = 挖掘光束在这里没有作用。

spell-drain-life-hit = { $target } suddenly seems weaker!

spell-drain-life-miss = 你没打中 { $target }。

spell-finger-of-death-kill = { $target } dies!

spell-finger-of-death-resisted = { $target } resists!

spell-haste-self = 你觉得自己动作变快了。

spell-healing = 你觉得好多了。

spell-identify = 你感到自己有了自知之明……

spell-insufficient-power = 你没有足够的魔力施放那个法术。

spell-invisibility = 你觉得自己有点飘飘然。

spell-jumping = 你跳了起来！

spell-jumping-blocked = 有什么东西挡住了你的跳跃。

spell-knock = 一扇门打开了！

spell-light = 一片照亮的区域环绕著你！

spell-magic-mapping = 周围环境的地图浮现在你眼前！

spell-need-direction = 往哪个方向？

spell-no-spellbook = 你没有可供研读的魔法书。

spell-polymorph-hit = { $target } undergoes a transformation!

ambient-court-conversation = 你听见宫廷式的谈话声。
ambient-court-judgment = 你听见权杖敲击作裁决的声音。
ambient-court-off-with-your-head = 你听见有人喊道：「把他脑袋砍下来！」
ambient-court-beruthiel = 你听见贝鲁希尔王后的猫叫声！
ambient-beehive-buzzing = 你听见低沉的嗡嗡声。
ambient-beehive-drone = 你听见愤怒的嗡鸣声。
ambient-beehive-bonnet = 你听见脑子里有蜜蜂在嗡嗡作响！
ambient-morgue-quiet = 你突然发现四周安静得不太自然。
ambient-morgue-neck-hair = 你后颈的汗毛都竖了起来。
ambient-morgue-head-hair = 你头上的头发似乎都竖了起来。
ambient-zoo-elephant = 你听见仿佛大象踩到花生的声音。
ambient-zoo-seal = 你听见仿佛海豹在吠叫的声音。
ambient-zoo-dolittle = 你听见了杜立德医生的声音！
ambient-oracle-woodchucks = 你听见有人说：「别再有土拨鼠了！」
ambient-oracle-zot = 你听见一声响亮的 ZOT！
ambient-vault-scrooge = 你听见了史古基的声音！
ambient-vault-quarterback = 你听见四分卫在呼喊战术。
ambient-shop-neiman-marcus = 你听见奈曼和马库斯在争吵！
spell-polymorph-miss = 你没打中 { $target }。

spell-protection-disappears = 你的金色光辉消散了。

spell-protection-less-dense = 你的金色雾气变淡了。

spell-remove-curse = 你感觉好像有人在帮你。

spell-restore-ability-nothing = 你感到短暂地神清气爽。

spell-restore-ability-restored = 哇！这让你感觉棒极了！

spell-sleep-hit = { $target } falls asleep!

spell-sleep-miss = { $target } resists!

spell-slow-monster-hit = { $target } seems to slow down.

spell-slow-monster-miss = { $target } resists!

spell-stone-to-flesh-cured = 你感到身体变得灵活！

spell-stone-to-flesh-nothing = 什么也没有发生。

spell-summon-insects = 你召唤出昆虫！

spell-summon-monster = 你召唤出一只怪物！

spell-teleport-away-hit = { $target } disappears!

spell-teleport-away-miss = { $target } resists!

spell-turn-undead-hit = { $target } flees!

spell-turn-undead-miss = { $target } resists!

spell-unknown = 你不认识那个法术。

spell-weaken = { $target } suddenly seems weaker!

spell-wizard-lock = 一扇门锁上了！

stairs-at-top = 你已在地城的顶层。

stairs-not-here = 这里看不见任何楼梯。

status-blindness-end = 你又能看见了。

status-confusion-end = 你感觉不那么混乱了。

status-fall-asleep = 你睡着了。

status-fumble-trip = 你被什么东西绊了一下。

status-fumbling-end = 你觉得自己没那么笨拙了。

status-fumbling-start = 你觉得自己笨手笨脚。

status-hallucination-end = 现在一切看起来都无聊透顶了。

status-invisibility-end = 你不再隐形了。

status-levitation-end = 你轻轻飘回了地面。

status-paralysis-end = 你又能动了。

status-paralyzed-cant-move = 你动弹不得！

status-sick-cured = 总算松了一口气！

status-sick-recovered = 你感觉好多了。

status-sleepy-end = 你清醒过来了。

status-sleepy-start = 你感到困倦。

status-speed-end = 你感觉自己慢了下来。

status-stun-end = 你觉得自己没那么晕了。

status-vomiting-end = 你觉得没那么恶心了。

status-vomiting-start = 你觉得一阵恶心。

status-wounded-legs-healed = 你的腿感觉好多了。

status-wounded-legs-start = 你的腿伤得很重！

steal-item-from-you = { $monster } steals { $item }!

steal-no-gold = { $monster } finds no gold on you.

steal-nothing-to-take = { $monster } finds nothing to steal.

steed-stops-galloping = 你的坐骑慢了下来，停住了。
steed-swims = 你的坐骑在水中划动前进。

stoning-limbs-stiffening = 你的四肢开始僵硬。

stoning-limbs-stone = 你的四肢已经变成石头。

stoning-slowing-down = 你动作开始变慢。

stoning-turned-to-stone = 你变成了石头。

stoning-you-are-statue = 你变成了一座雕像。

swap-no-secondary = 你没有副手武器。

swap-success = 你交换了武器。

swap-welded = 你的武器像焊住一样卡在手上！

swim-lava-burn = 你在岩浆里烧成了焦炭！

swim-water-paddle = 你在水中划动前进。

temple-eerie = 你感到一阵诡异的气氛……

temple-ghost-appears = 一个幽灵出现在你面前！

temple-shiver = 你打了个寒颤。

temple-watched = 你觉得好像有人在盯著你看。

throne-genocide = 一个声音回荡著：「汝当决定谁生谁死！」

throne-identify = 你感到自己有了自知之明……

throne-no-position = 你现在这个位置没法坐上去。

throne-not-here = 这里没有宝座。

throne-nothing = 看起来什么也没有发生。

throne-vanishes = 宝座在一阵逻辑的烟雾中消失了。

throne-wish = 一个声音回荡著：「汝之愿望已被应允！」

tip-cannot-reach = 你够不到那个。

tip-empty = 那个容器是空的。

tip-locked = 那个容器上锁了。
tip-dump = { $count ->
    [one] 一件物品从容器里倒了出来。
   *[other] 多件物品从容器里倒了出来。
}
tip-grease-spill = 一些油脂洒到了地面上。
tip-bag-of-tricks = { $count ->
    [one] 一个怪物从袋子里冒了出来！
   *[other] 多个怪物从袋子里冒了出来！
}

tool-bell-cursed-summon = 铃声召来了敌对生物！

tool-bell-cursed-undead = 铃声召来了不死生物！

tool-bell-no-sound = 但是声音被闷住了。

tool-bell-opens = 有什么东西打开了……

tool-bell-reveal = 你周围有东西打开了……

tool-bell-ring = 铃响了。

tool-bell-wake-nearby = 铃声唤醒了附近的怪物！

tool-bullwhip-crack = 你甩响了长鞭！

tool-camera-no-target = 没有什么可拍的。

tool-candelabrum-extinguish = 烛台上的蜡烛熄灭了。

tool-candelabrum-no-candles = 烛台上没有装蜡烛。

tool-candle-extinguish = 你把蜡烛熄掉了。

tool-candle-light = 你把蜡烛点亮了。

tool-cream-pie-face = 你的脸上糊满了奶油派！

tool-dig-no-target = 这里没有什么可挖的。

tool-drum-earthquake = 整座地城都在你周围摇晃！

tool-drum-no-charges = 鼓已经没电了。

tool-drum-thump = 你敲响了鼓。

tool-figurine-hostile = 雕像变成了敌对怪物！

tool-figurine-peaceful = 雕像变成了一只和平怪物。

tool-figurine-tame = 雕像变成了一只宠物！

tool-grease-empty = 润滑油罐已经空了。

tool-grease-hands = 你的手太滑了，什么都拿不住！

tool-grease-slip = 你上过油的 { $item } 滑掉了！

tool-horn-no-charges = 角已经没充能了。

tool-horn-toot = 你吹出了一声骇人而难听的声音。

tool-horn-of-plenty-spills = 丰饶之角里有什么东西洒了出来！

tool-leash-no-pet = 附近没有可拴绳的宠物。

tool-lockpick-breaks = 你的开锁器断掉了！

tool-magic-lamp-djinni = 一个灯神从灯里冒了出来！

tool-magic-whistle = 你吹出一声奇怪的口哨。

tool-mirror-self = 你照镜子时觉得自己丑极了。

tool-no-locked-door = 那里没有上锁的门。

tool-nothing-happens = 看起来什么也没有发生。

tool-polearm-no-target = 那里没有什么可打的。

tool-saddle-no-mount = 没有什么东西可以装上鞍。

tool-tin-whistle = 你吹出一声高亢的口哨。

tool-tinning-no-charges = 你似乎没有空罐头了。

tool-tinning-no-corpse = 这里没有可以装罐的尸体。

tool-touchstone-identify = 你把宝石在试金石上摩擦，辨认出了它们。

tool-touchstone-shatter = 宝石碎裂了！

tool-touchstone-streak = 宝石在试金石上留下了一道痕迹。

tool-towel-cursed-gunk = 你的脸上沾满了黏糊糊的东西！

tool-towel-cursed-nothing = 你没法把那些脏东西擦掉！

tool-towel-cursed-slimy = 你的脸摸起来黏黏的。

tool-towel-nothing = 你的脸已经很干净了。

tool-towel-wipe-face = 你把那团黏糊糊的东西擦掉了。

tool-unihorn-cured = 你感觉好多了！

tool-unihorn-cursed = 独角兽角被诅咒了！

tool-unihorn-nothing = 看起来什么也没有发生。

tool-unlock-fail = 你没能解开它。

tool-unlock-success = 你把它打开了。

tool-whistle-no-pets = 附近没有任何宠物。

tunnel-blocked = 这里没有挖掘的空间。

turn-no-undead = 没有不死生物可以驱离。

turn-not-clerical = 你不知道怎么驱离不死生物。

untrap-failed = 你没能解除陷阱。

untrap-no-trap = 你没找到任何陷阱。

untrap-success = 你解除陷阱了！

untrap-triggered = 你触发了陷阱！

vanquished-none = 目前还没有消灭任何怪物。

vault-guard-disappear = 守卫消失了。

vault-guard-escort = 守卫把你护送了出去。

vault-guard-state-change = 守卫改变了姿势。

vomiting-about-to = 你快要吐了。

vomiting-cant-think = 你脑子一团乱。

vomiting-incredibly-sick = 你觉得自己病得厉害。

vomiting-mildly-nauseated = 你感到有点反胃。

vomiting-slightly-confused = 你感到有些混乱。

vomiting-vomit = 你吐了！

wait = 时间流逝……

wand-cancel-monster = { $target } is covered by a shimmering light!

wand-digging-miss = 挖掘光束没命中。

wipe-cream-off = 你把脸上的奶油擦掉了。

wipe-cursed-towel = 毛巾被诅咒了！

wipe-nothing = 没有什么可擦掉的东西。

wizard-curse-items = 你感觉自己像是该找个驱魔人了。

wizard-detect-all = 你感知到四周的一切。

wizard-detect-monsters = 你感到仿佛有什么东西正在注视着你。

wizard-detect-objects = 你感知到附近有物体存在。

wizard-detect-traps = 你感到附近的陷阱正在向你发出警告。

wizard-double-trouble = "双重麻烦……"

wizard-identify-all = 你觉得自己对自己了如指掌……

wizard-genesis = 一只{ $monster }出现在你身旁。

wizard-genesis-failed = 没有什么回应你对{ $monster }的请求。

wizard-kill = 你抹除了本层中的{ $count }只怪物。

wizard-kill-none = 这里没有怪物可供抹除。

wizard-map-revealed = 你周围环境的景象在脑海中浮现！

wizard-vague-nervous = 你隐约感到不安。

wizard-black-glow = 你注意到一阵黑色光芒笼罩着你。

wizard-aggravate = 远处回荡起噪音，整座地城仿佛突然苏醒了。

wizard-respawned = 延德之巫再次站了起来！

wizard-respawned-boom = 一个声音轰然响起……

wizard-respawned-taunt = 蠢货，你竟以为自己能{$verb}我。

wizard-steal-amulet = 延德之巫偷走了护符！

wizard-steal-invocation-tool = 延德之巫偷走了其中一件祈唤道具！

wizard-steal-quest-artifact = 延德之巫偷走了任务神器！

wizard-summon-nasties = 新的恶物凭空出现了！

wizard-taunt-laughs = {$wizard} 发出阴森的狂笑。

wizard-taunt-relinquish = 交出护符吧，{$insult}！

wizard-taunt-panic = 即便此刻，你的生命力仍在流逝，{$insult}！

wizard-taunt-last-breath = 好好珍惜你的呼吸吧，{$insult}，那会是你最后一口气！

wizard-taunt-return = 我还会回来的。

wizard-taunt-back = 我会回来的。

wizard-taunt-general = {$malediction}，{$insult}！

amulet-feels-hot = 护符摸起来发烫！

amulet-feels-very-warm = 护符摸起来非常温热。

amulet-feels-warm = 护符摸起来温热。

wizard-where-current = 你现在位于 { $location }（绝对深度 { $absolute }）的 { $x },{ $y }。

wizard-where-special = { $level } lies on { $location }.

wizard-wish = 你的愿望实现了：{ $item }。

wizard-wish-adjusted = 你的愿望被调整为：{ $item }。

wizard-wish-adjusted-floor = 你的愿望被调整了：{ $item } 掉到了你脚边。

wizard-wish-failed = 没有任何东西回应你对「{ $wish }」的愿望。

wizard-wish-floor = 你的愿望实现了：{ $item } 掉到了你脚边。

worm-grows = 长虫又变长了！

worm-shrinks = 长虫缩短了！

worn-gauntlets-power-off = 你感到虚弱了。

worn-gauntlets-power-on = 你感到更强壮了！

worn-helm-brilliance-off = 你感到自己恢复平凡了。

npc-humanoid-threatens = { $monster } 威胁你。
npc-humanoid-avoid = { $monster } 一点也不想搭理你。
npc-humanoid-moans = { $monster } 呻吟着。
npc-humanoid-huh = { $monster } 说道：“哈？”
npc-humanoid-what = { $monster } 说道：“什么？”
npc-humanoid-eh = { $monster } 说道：“嗯？”
npc-humanoid-cant-see = { $monster } 说道：“我什么也看不见！”
npc-humanoid-trapped = { $monster } 说道：“我被困住了！”
npc-humanoid-healing = { $monster } 索要一瓶治疗药水。
npc-humanoid-hungry = { $monster } 说道：“我饿了。”
npc-humanoid-curses-orcs = { $monster } 咒骂兽人。
npc-humanoid-mining = { $monster } 谈论采矿。
npc-humanoid-spellcraft = { $monster } 谈论法术研究。
npc-humanoid-hunting = { $monster } 谈论狩猎。
npc-humanoid-gnome = { $monster } 说道：“走进地牢的人很多，能回到阳光下的却寥寥无几。”
npc-humanoid-gnome-phase-one = { $monster } 说道：“第一阶段，收集内裤。”
npc-humanoid-gnome-phase-three = { $monster } 说道：“第三阶段，盈利！”
npc-humanoid-hobbit-complains = { $monster } 抱怨地牢里的环境太糟糕。
npc-humanoid-one-ring = { $monster } 问你关于至尊魔戒的事。
npc-humanoid-aloha = { $monster } 说道：“阿罗哈。”
npc-humanoid-spelunker-today = { $monster } 讲起《今日洞穴探险家》杂志上的一篇近文。
npc-humanoid-dungeon-exploration = { $monster } 谈论地牢探险。
npc-boast-gem-collection = { $monster } 吹嘘自己的宝石收藏。
npc-boast-mutton = { $monster } 抱怨自己只能吃羊肉。
npc-boast-fee-fie-foe-foo = { $monster } 高喊“Fee Fie Foe Foo！”并放声大笑。
npc-arrest-facts-maam = { $monster } 说道：“只说事实，女士。”
npc-arrest-facts-sir = { $monster } 说道：“只说事实，先生。”
npc-arrest-anything-you-say = { $monster } 说道：“你说的每句话都可能成为呈堂证供。”
npc-arrest-under-arrest = { $monster } 说道：“你被捕了！”
npc-arrest-stop-law = { $monster } 说道：“以法律的名义，停下！”
npc-djinni-no-wishes = { $monster } 说道：“抱歉，我已经没愿望可许了。”
npc-djinni-free = { $monster } 说道：“我自由了！”
npc-djinni-get-me-out = { $monster } 说道：“快把我弄出去。”
npc-djinni-disturb = { $monster } 说道：“这下你知道打扰我是什么下场了！”
npc-cuss-curses = { $monster } 破口大骂。
npc-cuss-imprecates = { $monster } 恶毒咒骂。
npc-cuss-not-too-late = { $monster } 说道：“现在还不算太晚。”
npc-cuss-doomed = { $monster } 说道：“我们全都完了。”
npc-cuss-ancestry = { $monster } 诋毁你的出身。
npc-cuss-angel-repent = { $monster } 说道：“悔改吧，你就能得救！”
npc-cuss-angel-insolence = { $monster } 说道：“你的无礼会付出代价！”
npc-cuss-angel-maker = { $monster } 说道：“很快了，孩子，你将见到你的造物主。”
npc-cuss-angel-wrath = { $monster } 说道：“天罚现在降临到你身上！”
npc-cuss-angel-not-worthy = { $monster } 说道：“你没有资格寻找护符。”
npc-cuss-demon-slime = { $monster } 说道：“吃泥浆去死吧！”
npc-cuss-demon-clumsy = { $monster } 说道：“你是喝醉了，还是平时就这么笨手笨脚？”
npc-cuss-demon-laughter = { $monster } 说道：“饶了我吧！你是想让我笑死吗？”
npc-cuss-demon-amulet = { $monster } 说道：“为什么要找护符？你只会把它丢掉，白痴。”
npc-cuss-demon-comedian = { $monster } 说道：“你的本事这么可笑，干脆去当喜剧演员吧！”
npc-cuss-demon-odor = { $monster } 说道：“你有没有想过遮一遮你那股味道？”
demon-demand-safe-passage = { $monster } 索要 { $amount } 枚佐克币作为买路钱。
demon-demand-something = { $monster } 似乎在索要什么。
demon-offer-all-gold = 你把所有金币都给了 { $monster }。
demon-offer-amount = 你给了 { $monster } { $amount } 枚佐克币。
demon-refuse = 你拒绝了。
demon-shortchange = 你想少给 { $monster }，却手忙脚乱。
demon-vanishes-laughing = { $monster } 带着对凡人胆怯的嘲笑消失了。
demon-scowls-vanishes = { $monster } 恶狠狠地瞪了你一眼，随后消失。
demon-gets-angry = { $monster } 生气了……
demon-good-hunting = { $monster } 说道：“祝你好运，{ $honorific }。”
demon-says-something = { $monster } 嘟囔了些什么。
demon-looks-angry = { $monster } 看上去非常愤怒。
demon-tension-building = 你感到气氛越来越紧张。
npc-spell-cantrip = { $monster } 似乎在低声念着小咒。
npc-vampire-tame-craving = { $monster } 说道：“我再也受不了这股渴望了！”
npc-vampire-tame-night-craving = { $monster } 说道：“求你帮我满足这越来越强的渴望！”
npc-vampire-tame-weary = { $monster } 说道：“我感觉有点累了。”
npc-vampire-tame-kindred-evening = { $monster } 说道：“夜安，主人！”
npc-vampire-tame-kindred-day = { $monster } 说道：“日安，主人。我们为何不休息？”
npc-vampire-tame-nightchild-craving = { $monster } 说道：“夜之子啊，我再也受不了这股渴望了！”
npc-vampire-tame-nightchild-night-craving = { $monster } 说道：“夜之子啊，求你帮我满足这越来越强的渴望！”
npc-vampire-tame-nightchild-weary = { $monster } 说道：“夜之子啊，我感觉有点累了。”
npc-vampire-peaceful-kindred-sister = { $monster } 说道：“祝你喂食愉快，姐妹！”
npc-vampire-peaceful-kindred-brother = { $monster } 说道：“祝你喂食愉快，兄弟！”
npc-vampire-peaceful-nightchild = { $monster } 说道：“听到你说话真好，夜之子。”
npc-vampire-peaceful = { $monster } 说道：“我只喝……药水。”
npc-vampire-hostile-hunting-ground = { $monster } 说道：“这可是我的猎场，你竟敢在这里游荡！”
npc-vampire-hostile-silver-dragon = { $monster } 说道：“蠢货！你那点银光吓不倒我！”
npc-vampire-hostile-baby-silver-dragon = { $monster } 说道：“小蠢货！你那点银光吓不倒我！”
npc-vampire-hostile-blood = { $monster } 说道：“我要吸干你的血！”
npc-vampire-hostile-hunt = { $monster } 说道：“我会毫无顾忌地追杀你！”
npc-imitate-imitates = { $monster } 学着你的样子。
npc-rider-sandman = { $monster } 正忙着看一本《梦神》#8。
npc-rider-war = { $monster } 说道：“你以为你是谁，战争吗？”
npc-seduce-hello-sailor = { $monster } 说道：“嗨，水手。”
npc-seduce-comes-on = { $monster } 在勾引你。
npc-seduce-cajoles = { $monster } 在对你甜言蜜语。
npc-nurse-put-weapon-away = { $monster } 说道：“把武器收起来，免得伤到人！”
npc-nurse-doc-cooperate = { $monster } 说道：“医生，你不配合我可没法帮你。”
npc-nurse-please-undress = { $monster } 说道：“请脱下衣服，让我检查一下。”
npc-nurse-take-off-shirt = { $monster } 说道：“请把上衣脱了。”
npc-nurse-relax = { $monster } 说道：“放轻松，不会疼的。”
npc-guard-drop-gold = { $monster } 说道：“请把金币放下，然后跟我走。”
npc-guard-follow-me = { $monster } 说道：“请跟我来。”
npc-soldier-pay = { $monster } 说道：“这里的军饷也太寒酸了！”
npc-soldier-food = { $monster } 说道：“这食物连兽人都不想吃！”
npc-soldier-feet = { $monster } 说道：“我的脚都站痛了，已经忙了一整天！”
npc-soldier-resistance = { $monster } 说道：“抵抗是没用的！”
npc-soldier-dog-meat = { $monster } 说道：“你就是砧板上的肉！”
npc-soldier-surrender = { $monster } 说道：“投降吧！”

worn-helm-brilliance-on = 你觉得自己灵光一现！

write-no-marker = 你没有魔法记号笔。

write-not-enough-charges = 你的记号笔已经干得写不出来了！

write-scroll-fail-daiyen-fansen = 你的记号笔彻底干掉了！

write-spellbook-fail = 那本法术书古怪地扭曲了一下，随后变成了一片空白。

write-spellbook-success = 你成功写好了这本法术书！

priest-not-enough-gold = 牧师向你索要 { $cost } 枚金币。

priest-protection-granted = 牧师以 { $cost } 枚金币为你提供庇护。

shk-welcome = { $shopkeeper } 说道：“欢迎光临我的店，{ $honorific }。”

shk-angry-greeting = { $shopkeeper } 愤怒地瞪着你。
shk-angry-rude = { $shopkeeper } 示意自己很讨厌粗鲁的顾客。
shk-angry-rude-indicates = { $shopkeeper } 明白表示，这里一点也不欢迎粗鲁的顾客。
shk-angry-non-paying = { $shopkeeper } 示意自己很讨厌不付钱的顾客。
shk-angry-non-paying-indicates = { $shopkeeper } 明白表示，这里一点也不欢迎不付钱的顾客。

shk-follow-reminder = { $shopkeeper } 说道：“您好，{ $honorific }！您是不是忘了付账？”

shk-follow-tap = { $shopkeeper } 轻轻拍了拍你的手臂。

shk-bill-total = { $shopkeeper } 说你的账单一共是 { $amount } 枚金币。

shk-bill-indicates = { $shopkeeper } 示意你的账单一共是 { $amount } 枚金币。

shk-debit-reminder = { $shopkeeper } 提醒你还欠 { $amount } 枚金币。

shk-debit-indicates = { $shopkeeper } 示意你还欠 { $amount } 枚金币。

shk-credit-reminder = { $shopkeeper } 提醒你可以使用你那 { $amount } 枚金币的赊账额度。

shk-credit-indicates = { $shopkeeper } 示意你还有 { $amount } 枚金币的信用额度。

shk-robbed-greeting = { $shopkeeper } 说道：“那次抢劫我还记得清清楚楚呢，{ $honorific }。”
shk-robbed-indicates = { $shopkeeper } 示意自己还在担心最近那次抢劫。
shk-surcharge-greeting = { $shopkeeper } 说道：“你现在得付更高的价钱了，{ $honorific }。”
shk-surcharge-indicates = { $shopkeeper } 示意你现在要付更高的价格。
shk-business-bad = { $shopkeeper } 抱怨生意不好。
shk-business-bad-indicates = { $shopkeeper } 示意自己最近生意不景气。
shk-business-good = { $shopkeeper } 说最近生意不错。
shk-business-good-indicates = { $shopkeeper } 示意自己最近生意兴隆。

shk-shoplifters = { $shopkeeper } 抱怨商店扒手的问题。

shk-shoplifters-indicates = { $shopkeeper } 示意自己很担心店里的扒手。

shk-geico-pitch = { $shopkeeper } 说道：“十五分钟就能为你省下十五枚佐克币。”
shk-izchak-malls = { $shopkeeper } 说道：“这些商场真让我头疼。”
shk-izchak-slow-down = { $shopkeeper } 说道：“慢一点，想清楚再说。”
shk-izchak-one-at-a-time = { $shopkeeper } 说道：“事情得一件一件来。”
shk-izchak-coffee = { $shopkeeper } 说道：“我不喜欢花里胡哨的咖啡……给我哥伦比亚顶级咖啡。”
shk-izchak-devteam = { $shopkeeper } 说道，要让开发团队就任何事达成一致都很难。
shk-izchak-deity = { $shopkeeper } 说道，侍奉自己神祇的人会有福报。
shk-izchak-high-places = { $shopkeeper } 说道：“别想偷我的东西，我可是上头有人！”
shk-izchak-future = { $shopkeeper } 说道，今后你很可能还会需要这家店里的东西。
shk-izchak-valley = { $shopkeeper } 评论说，亡者之谷本就是一处通道。

npc-laugh-giggles = { $monster } 咯咯直笑。
npc-laugh-chuckles = { $monster } 吃吃地笑了起来。
npc-laugh-snickers = { $monster } 窃笑起来。
npc-laugh-laughs = { $monster } 笑了起来。

npc-gecko-geico-pitch = { $monster } 说道：“十五分钟就能为你省下十五枚佐克币。”

npc-mumble-incomprehensible = { $monster } 咕哝着说些听不懂的话。

npc-bones-rattle = { $monster } 哗啦哗啦地响个不停。

npc-shriek = { $monster } 尖声嘶叫。

npc-bark-barks = { $monster } 汪汪叫。

npc-bark-whines = { $monster } 呜咽起来。

npc-bark-howls = { $monster } 嚎叫起来。

npc-bark-yips = { $monster } 短促地叫了一声。

npc-mew-mews = { $monster } 轻轻喵了一声。

npc-mew-yowls = { $monster } 凄厉地嚎叫起来。

npc-mew-meows = { $monster } 喵喵叫。

npc-mew-purrs = { $monster } 发出呼噜声。

npc-growl-growls = { $monster } 发出低吼！

npc-growl-snarls = { $monster } 龇牙低吼。

npc-roar-roars = { $monster } 发出咆哮！

npc-bellow-bellows = { $monster } 发出怒吼！

npc-squeak-squeaks = { $monster } 吱吱叫。

npc-squawk-squawks = { $monster } 嘎嘎乱叫。

npc-squawk-nevermore = { $monster } 说道：“永不复来！”

npc-chirp-chirps = { $monster } 啾啾叫。

npc-hiss-hisses = { $monster } 发出嘶嘶声！

npc-buzz-drones = { $monster } 嗡嗡作响。

npc-buzz-angry = { $monster } 愤怒地嗡嗡作响。

npc-grunt-grunts = { $monster } 哼了一声。

npc-neigh-neighs = { $monster } 嘶鸣起来。

npc-neigh-whinnies = { $monster } 嘶鸣不已。

npc-neigh-whickers = { $monster } 发出轻快的马鸣。

npc-were-shrieks = { $monster } 发出令人毛骨悚然的尖叫！

npc-were-howls = { $monster } 发出令人毛骨悚然的嚎叫！

npc-were-moon = { $monster } 低声耳语，几乎听不清。你只勉强分辨出“月亮”两个字。

npc-moo-moos = { $monster } 哞哞叫。

npc-wail-wails = { $monster } 哀号起来。

npc-gurgle-gurgles = { $monster } 发出咕噜声。

npc-burble-burbles = { $monster } 咕哝作响。

npc-trumpet-trumpets = { $monster } 发出高亢的号鸣。

npc-groan-groans = { $monster } 呻吟起来。

god-roars-suffer = 一个轰鸣的声音咆哮道：“为你的亵渎受苦吧！”

god-how-dare-harm-servant = 一个轰鸣的声音咆哮道：“你竟敢伤害我的仆从？”

god-profane-shrine = 一个轰鸣的声音咆哮道：“你玷污了我的神殿！”
ambient-gehennom-damned = 你听到被诅咒者的哀嚎！
ambient-gehennom-groans = 你听到呻吟与哀号！
ambient-gehennom-laughter = 你听到恶魔般的狞笑！
ambient-gehennom-brimstone = 你闻到硫磺味！
ambient-mines-money = 你听到有人在数钱。
ambient-mines-register = 你听到收银机的叮当声。
ambient-mines-cart = 你听到仿佛吃力矿车发出的声响。
ambient-shop-shoplifters = 你听到有人在咒骂商店扒手。
ambient-shop-register = 你听到收银机的叮当声。
ambient-shop-prices = 你听到有人在小声嘟囔价格。
ambient-temple-praise = 你听到有人在赞颂 { $deity }。
ambient-temple-beseech = 你听到有人在恳求 { $deity }。
ambient-temple-sacrifice = 你听到有人献上动物尸体作为祭品。
ambient-temple-donations = 你听到有人高声乞求捐献。
ambient-oracle-wind = 你听到一阵诡异的风声。
ambient-oracle-ravings = 你听到一阵痉挛般的胡言乱语。
ambient-oracle-snakes = 你听到蛇的鼾声。
ambient-barracks-honed = 你听到有人在磨刀。
ambient-barracks-snoring = 你听到响亮的鼾声。
ambient-barracks-dice = 你听到掷骰子的声音。
ambient-barracks-macarthur = 你仿佛听到了麦克阿瑟将军的声音！
ambient-swamp-mosquitoes = 你听到蚊群的嗡嗡声！
ambient-swamp-marsh-gas = 你闻到沼气味！
ambient-swamp-donald-duck = 你仿佛听到了唐老鸭的叫声！
ambient-fountain-bubbling = 你听到汩汩的水声。
ambient-fountain-coins = 你听到水滴落在硬币上的声音。
ambient-fountain-naiad = 你听到水泽仙女拍水的声音。
ambient-fountain-soda = 你听到汽水喷泉般的声响！
ambient-sink-drip = 你听到水滴缓缓落下的声音。
ambient-sink-gurgle = 你听到咕噜咕噜的水声。
ambient-sink-dishes = 你听到有人在洗盘子！
ambient-vault-counting = 你听到有人在数钱。
ambient-vault-searching = 你听到有人在翻找什么。
ambient-vault-footsteps = 你听到巡逻守卫的脚步声。
ambient-deep-crunching = 你听到咔嚓咔嚓的碎裂声。
ambient-deep-hollow = 你听到空空回响的声音。
ambient-deep-rumble = 你听到低沉的隆隆声。
ambient-deep-roar = 你听到远处传来一声咆哮。
ambient-deep-digging = 你听到有人在挖掘。
ambient-shallow-door-open = 你听到门被打开的声音。
ambient-shallow-door-close = 你听到门被关上的声音。
ambient-shallow-water = 你听到滴水声。
ambient-shallow-moving = 你听到附近有人在走动。
ui-wizard-mode-disabled = 未启用巫师模式。
ui-item-prompt-drop = 丢弃哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-wield = 挥舞哪一件？[a-zA-Z 或 - 表示空手]
ui-item-prompt-wear = 穿戴哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-take-off = 脱下哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-put-on = 戴上哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-remove = 取下哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-apply = 使用哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-ready = 准备哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-throw = 投掷哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-zap = 击发哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-invoke = 调用哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-rub = 摩擦哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-tip = 倾倒哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-offer = 献上哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-force-lock = 用哪一件强行撬锁？[a-zA-Z 或 ?*]
ui-item-prompt-dip = 蘸哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-dip-into = 蘸进哪一件里？[a-zA-Z 或 ?*]
ui-item-prompt-name-item = 给哪件物品命名？[a-zA-Z 或 ?*]
ui-item-prompt-adjust-item = 调整哪件物品？[a-zA-Z 或 ?*]
ui-text-prompt-wish = 想许什么愿？
ui-text-prompt-create-monster = 要创建哪种怪物？
ui-text-prompt-teleport-level = 要传送到地城第几层？
ui-text-prompt-annotate-level = 给这一层加什么注记：
ui-text-prompt-engrave = 要刻写什么？
ui-text-prompt-call-class = 要给哪个类别字母命名？
ui-text-prompt-call-name = 要叫它什么？
ui-text-prompt-known-class = 查看哪个类别字母？
ui-text-prompt-cast-spell = 施放哪个法术字母？
ui-text-prompt-name-target = 命名目标（[i]物品/[m]怪物/[l]楼层）？
ui-text-prompt-name-level = 给这一层起名：
ui-text-prompt-call-monster = 要叫这只怪物什么？
ui-text-prompt-name-it = 要给它起什么名字？
ui-text-prompt-assign-inventory-letter = 指定新的物品栏字母：
ui-position-prompt-travel = 要前往哪里？
ui-position-prompt-jump = 要跳到哪里？
ui-position-prompt-inspect = 检查哪个位置？
ui-position-prompt-look = 查看哪个位置？
ui-position-prompt-name-monster = 给哪个位置的怪物命名？
ui-equipment-slot-weapon = 武器
ui-equipment-slot-off-hand = 副手
ui-equipment-slot-helmet = 头盔
ui-equipment-slot-cloak = 披风
ui-equipment-slot-body-armor = 身体护甲
ui-equipment-slot-shield = 盾牌
ui-equipment-slot-gloves = 手套
ui-equipment-slot-boots = 靴子
ui-equipment-slot-shirt = 内衬
ui-equipment-slot-ring-left = 戒指（左）
ui-equipment-slot-ring-right = 戒指（右）
ui-equipment-slot-amulet = 护身符
ui-game-over-title = *** 游戏结束 ***
ui-game-over-score-line = 分数：{ $score }
ui-game-over-cause-line = 死因：{ $cause }
ui-game-over-turns-line = 回合数：{ $turns }
ui-extcmd-desc-hash = 输入并执行扩展命令
ui-extcmd-desc-question = 列出所有扩展命令
ui-extcmd-desc-apply = 使用一件工具
ui-extcmd-desc-cast = 施放法术
ui-extcmd-desc-chat = 与某人交谈
ui-extcmd-desc-close = 关闭一扇门
ui-extcmd-desc-dip = 把一件物品浸入别的东西
ui-extcmd-desc-drop = 丢下一件物品
ui-extcmd-desc-eat = 吃东西
ui-extcmd-desc-fight = 朝一个方向攻击
ui-extcmd-desc-glance = 查看地图符号表示什么
ui-extcmd-desc-inventory = 查看你的物品栏
ui-extcmd-desc-jump = 跳到一个位置
ui-extcmd-desc-kick = 踢某个方向
ui-extcmd-desc-loot = 搜刮容器或怪物
ui-extcmd-desc-look = 描述这里有什么
ui-extcmd-desc-open = 打开一扇门
ui-extcmd-desc-offer = 献上祭品
ui-extcmd-desc-pickup = 捡起这里的物品
ui-extcmd-desc-pray = 向你的神祈祷
ui-extcmd-desc-quiver = 准备要发射的弹药
ui-extcmd-desc-quit = 退出游戏
ui-extcmd-desc-save = 保存游戏
ui-extcmd-desc-search = 搜索隐藏事物
ui-extcmd-desc-sit = 坐下
ui-extcmd-desc-throw = 投掷一件物品
ui-extcmd-desc-travel = 前往地图上的某个位置
ui-extcmd-desc-untrap = 拆除陷阱或装置
ui-extcmd-desc-wear = 穿上一件护甲
ui-extcmd-desc-whatis = 描述地图符号或位置
ui-extcmd-desc-wield = 挥舞一件武器
ui-extcmd-desc-zap = 击发一根魔杖
