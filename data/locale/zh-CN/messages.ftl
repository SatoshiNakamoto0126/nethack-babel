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
shop-price = "给你，{ $item }只要{ $price }枚金币。"
shop-stolen = 你有未付款的商品！
shop-damage = { $shopkeeper }说"你得赔偿损失！"
shop-shoplift = { $shopkeeper }尖叫道："站住，小偷！"
temple-enter = 你进入了{ $god }的神殿。
temple-donate = { $priest }接受了你的捐赠。
temple-protection = { $priest }赐予你神圣的保护。
vault-guard = 突然，一名金库守卫出现了！
vault-guard-ask = "你是谁？你在这里做什么？"

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
shop-angry-take = "谢谢你，贱人！"
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
terrain-wall = 墙壁
terrain-closed-door = 关闭的门
terrain-open-door = 打开的门
terrain-stairs-up = 向上的楼梯
terrain-stairs-down = 向下的楼梯
terrain-fountain = 喷泉
terrain-altar = 祭坛
terrain-water = 水
terrain-lava = 岩浆
terrain-trap = 陷阱
terrain-tree = 树
terrain-iron-bars = 铁栅栏

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
chat-nobody-there = 那里没有人可以交谈。

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

## ============================================================================
## 内容传递（谣言、神谕）
## ============================================================================

rumor-fortune-cookie = 你打开了幸运饼干，上面写着："{ $rumor }"
oracle-consultation = { $text }

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

# TODO: translate
already-mounted = You are already riding.

# TODO: translate
already-punished = You are already being punished.

# TODO: translate
attack-acid-hit = You are splashed by acid!

# TODO: translate
attack-acid-resisted = The acid doesn't seem to affect you.

# TODO: translate
attack-breath = { $monster } breathes at you!

# TODO: translate
attack-cold-hit = You are covered in frost!

# TODO: translate
attack-cold-resisted = You feel mildly chilly.

# TODO: translate
attack-disease = You feel very sick.

# TODO: translate
attack-disintegrate = You are disintegrated!

# TODO: translate
attack-disintegrate-resisted = You are not disintegrated.

# TODO: translate
attack-drain-level = You feel your life force draining away!

# TODO: translate
attack-engulf = { $monster } engulfs you!

# TODO: translate
attack-fire-hit = You are engulfed in flames!

# TODO: translate
attack-fire-resisted = You feel mildly warm.

# TODO: translate
attack-hug-crush = You are being crushed!

# TODO: translate
attack-paralyze = You are frozen in place!

# TODO: translate
attack-poisoned = You feel very sick!

# TODO: translate
attack-shock-hit = You are jolted by electricity!

# TODO: translate
attack-shock-resisted = You are only mildly tingled.

# TODO: translate
attack-sleep = You feel drowsy...

# TODO: translate
attack-slowed = You feel yourself moving more slowly.

# TODO: translate
attack-stoning-start = You are starting to turn to stone!

# TODO: translate
boulder-blocked = The boulder is wedged in.

# TODO: translate
boulder-fills-pit = The boulder fills a pit!

# TODO: translate
boulder-push = You push the boulder.

# TODO: translate
call-empty-name = You didn't give a name.

# TODO: translate
cannot-do-that = You can't do that.

# TODO: translate
choke-blood-trouble = You find it hard to breathe.

# TODO: translate
choke-consciousness-fading = Your consciousness is fading...

# TODO: translate
choke-gasping-for-air = You are gasping for air!

# TODO: translate
choke-hard-to-breathe = You find it hard to breathe.

# TODO: translate
choke-neck-constricted = Your neck is being constricted!

# TODO: translate
choke-neck-pressure = You feel pressure on your neck.

# TODO: translate
choke-no-longer-breathe = You can no longer breathe.

# TODO: translate
choke-suffocate = You suffocate.

# TODO: translate
choke-turning-blue = You're turning blue.

# TODO: translate
chronicle-empty = Your chronicle is empty.

# TODO: translate
clairvoyance-nothing-new = You sense nothing new.

# TODO: translate
container-put-in = You put { $item } into { $container }.

# TODO: translate
container-take-out = You take { $item } out of { $container }.

# TODO: translate
crystal-ball-cloudy = All you see is a swirling mess.

# TODO: translate
crystal-ball-nothing-new = You see nothing new.

# TODO: translate
cursed-cannot-remove = You can't remove it, it's cursed!

# TODO: translate
detect-food-none = You don't sense any food.

# TODO: translate
detect-gold-none = You don't sense any gold.

# TODO: translate
detect-monsters-none = You don't sense any monsters.

# TODO: translate
detect-objects-none = You don't sense any objects.

# TODO: translate
detect-traps-none = You don't sense any traps.

# TODO: translate
dig-blocked = This is too hard to dig in.

# TODO: translate
dig-floor-blocked = The floor here is too hard to dig in.

# TODO: translate
dig-floor-hole = You dig a hole through the floor!

# TODO: translate
dig-ray-nothing = The digging ray has no effect.

# TODO: translate
dig-wall-done = You finish digging through the wall.

# TODO: translate
dip-acid-nothing = Nothing happens.

# TODO: translate
dip-acid-repair = Your { $item } looks as good as new!

# TODO: translate
dip-amethyst-cure = You feel less confused.

# TODO: translate
dip-diluted = Your { $item } is diluted.

# TODO: translate
dip-excalibur = As you dip the sword, a strange light plays over it! Your sword is now named Excalibur!

# TODO: translate
dip-fountain-cursed = The water glows for a moment.

# TODO: translate
dip-fountain-nothing = Nothing seems to happen.

# TODO: translate
dip-fountain-rust = Your { $item } rusts!

# TODO: translate
dip-holy-water = You dip your { $item } in the holy water.

# TODO: translate
dip-no-fountain = There is no fountain here to dip into.

# TODO: translate
dip-not-a-potion = That's not a potion!

# TODO: translate
dip-nothing-happens = Nothing seems to happen.

# TODO: translate
dip-poison-weapon = You coat your { $item } with poison.

# TODO: translate
dip-unholy-water = You dip your { $item } in the unholy water.

# TODO: translate
dip-unicorn-horn-cure = You feel better.

# TODO: translate
djinni-from-bottle = An enormous djinni emerges from the bottle!

# TODO: translate
drawbridge-destroyed = The drawbridge is destroyed!

# TODO: translate
drawbridge-lowers = The drawbridge lowers!

# TODO: translate
drawbridge-raises = The drawbridge raises!

# TODO: translate
drawbridge-resists = The drawbridge resists!

# TODO: translate
end-ascension-offering = You offer the Amulet of Yendor to { $god }...

# TODO: translate
end-do-not-pass-go = Do not pass go. Do not collect 200 zorkmids.

# TODO: translate
engrave-elbereth = You engrave "Elbereth" into the floor.

# TODO: translate
engulf-ejected = You are expelled from { $monster }!

# TODO: translate
engulf-escape-killed = You kill { $monster } from inside!

# TODO: translate
fire-no-ammo = You have nothing appropriate to fire.

# TODO: translate
fountain-chill = You feel a sudden chill.

# TODO: translate
fountain-curse-items = A feeling of loss comes over you.

# TODO: translate
fountain-dip-curse = The water glows for a moment.

# TODO: translate
fountain-dip-nothing = Nothing seems to happen.

# TODO: translate
fountain-dip-uncurse = The water glows for a moment.

# TODO: translate
fountain-dried-up = The fountain has dried up!

# TODO: translate
fountain-dries-up = The fountain dries up!

# TODO: translate
fountain-find-gem = You feel a gem here!

# TODO: translate
fountain-foul = The water is foul! You gag and vomit.

# TODO: translate
fountain-gush = Water gushes forth from the overflowing fountain!

# TODO: translate
fountain-no-position = You can't dip from this position.

# TODO: translate
fountain-not-here = There is no fountain here.

# TODO: translate
fountain-nothing = A large bubble rises to the surface and pops.

# TODO: translate
fountain-poison = The water is contaminated!

# TODO: translate
fountain-refresh = The cool draught refreshes you.

# TODO: translate
fountain-see-invisible = You feel self-knowledgeable...

# TODO: translate
fountain-see-monsters = You feel the presence of evil.

# TODO: translate
fountain-self-knowledge = You feel self-knowledgeable...

# TODO: translate
fountain-shimmer = You see a shimmering pool.

# TODO: translate
fountain-tingling = A strange tingling runs up your arm.

# TODO: translate
fountain-water-demon = An endless stream of snakes pours forth!

# TODO: translate
fountain-water-moccasin = An endless stream of snakes pours forth!

# TODO: translate
fountain-water-nymph = A wisp of vapor escapes the fountain...

# TODO: translate
ghost-from-bottle = As you open the bottle, something emerges.

# TODO: translate
god-lightning-bolt = Suddenly, a bolt of lightning strikes you!

# TODO: translate
grave-corpse = You find a corpse in the grave.

# TODO: translate
grave-empty = The grave is unoccupied. Strange...

# TODO: translate
guard-halt = "Halt, thief! You're under arrest!"

# TODO: translate
guard-no-gold = The guard finds no gold on you.

# TODO: translate
guardian-angel-appears = 一位守护天使出现在你身旁！
guardian-angel-rebukes = Your guardian angel rebukes you!

# TODO: translate
hunger-faint = You faint from lack of food.

# TODO: translate
hunger-starvation = You die from starvation.

# TODO: translate
instrument-no-charges = The instrument is out of charges.

# TODO: translate
intrinsic-acid-res-temp = You feel a momentary tingle.

# TODO: translate
intrinsic-cold-res = You feel full of hot air.

# TODO: translate
intrinsic-disint-res = You feel very firm.

# TODO: translate
intrinsic-fire-res = You feel a momentary chill.

# TODO: translate
intrinsic-invisibility = You feel rather airy.

# TODO: translate
intrinsic-poison-res = You feel healthy.

# TODO: translate
intrinsic-see-invisible = You feel perceptive!

# TODO: translate
intrinsic-shock-res = Your health currently feels amplified!

# TODO: translate
intrinsic-sleep-res = You feel wide awake.

# TODO: translate
intrinsic-stone-res-temp = You feel unusually limber.

# TODO: translate
intrinsic-strength = You feel strong!

# TODO: translate
intrinsic-telepathy = You feel a strange mental acuity.

# TODO: translate
intrinsic-teleport-control = You feel in control of yourself.

# TODO: translate
intrinsic-teleportitis = You feel very jumpy.

# TODO: translate
invoke-no-power = Nothing seems to happen.

# TODO: translate
invoke-not-wielded = You must be wielding it to invoke its power.

# TODO: translate
jump-no-ability = You don't know how to jump.

# TODO: translate
jump-out-of-range = That spot is too far away!

# TODO: translate
jump-success = You jump!

# TODO: translate
jump-too-burdened = You are carrying too much to jump!

# TODO: translate
kick-door-held = The door is held shut!

# TODO: translate
kick-door-open = The door crashes open!

# TODO: translate
kick-hurt-foot = Ouch! That hurts!

# TODO: translate
kick-item-blocked = Something blocks your kick.

# TODO: translate
kick-item-moved = You kick something.

# TODO: translate
kick-nothing = You kick at empty space.

# TODO: translate
kick-sink-ring = Something rattles around in the sink.

# TODO: translate
known-nothing = You don't know anything yet.

# TODO: translate
levitating-cant-go-down = You are floating high above the floor.

# TODO: translate
levitating-cant-pickup = You cannot reach the floor.

# TODO: translate
levitation-float-lower = You float gently to the floor.

# TODO: translate
levitation-wobble = You wobble in midair.

# TODO: translate
light-extinguished = Your { $item } goes out.

# TODO: translate
light-lit = Your { $item } is now lit.

# TODO: translate
light-no-fuel = Your { $item } has no fuel.

# TODO: translate
light-not-a-source = That is not a light source.

# TODO: translate
lizard-cures-confusion = You feel less confused.

# TODO: translate
lizard-cures-stoning = You feel limber!

# TODO: translate
lock-already-locked = It is already locked.

# TODO: translate
lock-door-locked = The door is locked.

# TODO: translate
lock-force-container-success = You force the lock open!

# TODO: translate
lock-force-fail = You fail to force the lock.

# TODO: translate
lock-force-success = You force the lock open!

# TODO: translate
lock-lockpick-breaks = Your lockpick breaks!

# TODO: translate
lock-need-key = You need a key to lock this.

# TODO: translate
lock-no-door = There is no door here.

# TODO: translate
lock-no-target = There is nothing here to lock or unlock.

# TODO: translate
lock-pick-container-success = You succeed in picking the lock.

# TODO: translate
lock-pick-fail = You fail to pick the lock.

# TODO: translate
lock-pick-success = You succeed in picking the lock.

# TODO: translate
lycanthropy-cured = You feel purified.

# TODO: translate
lycanthropy-full-moon-transform = You feel feverish tonight.

# TODO: translate
lycanthropy-infected = You feel feverish.

# TODO: translate
magic-mapping-nothing-new = You are already aware of your surroundings.

# TODO: translate
mhitm-passive-stoning = { $monster } turns to stone!

# TODO: translate
monster-ability-used = { $monster } uses a special ability!

# TODO: translate
monster-no-ability = You don't have that ability in your current form.

# TODO: translate
monster-not-polymorphed = You are not polymorphed.

# TODO: translate
monster-scared-elbereth = { $monster } is scared by the Elbereth engraving!

# TODO: translate
monster-teleport-near = { $monster } appears from thin air!

# TODO: translate
mount-not-monster = That is not a creature you can ride.

# TODO: translate
mount-not-tame = That creature is not tame enough to ride.

# TODO: translate
mount-too-far = That creature is too far away.

# TODO: translate
mount-too-weak = You are too weak to ride.

# TODO: translate
no-fountain-here = There is no fountain here.

# TODO: translate
not-a-drawbridge = That is not a drawbridge.

# TODO: translate
not-a-raised-drawbridge = That drawbridge is not raised.

# TODO: translate
not-carrying-anything = You are not carrying anything.

# TODO: translate
not-mounted = You are not riding anything.

# TODO: translate
not-punished = You are not being punished.

# TODO: translate
not-wearing-that = You are not wearing that.

# TODO: translate
phaze-feeling-bloated = You feel bloated.

# TODO: translate
phaze-feeling-flabby = You feel flabby.

# TODO: translate
play-bugle = You play the bugle.

# TODO: translate
play-drum = You beat the drum.

# TODO: translate
play-earthquake = The entire dungeon is shaking around you!

# TODO: translate
play-horn-noise = You produce a frightful, horrible sound.

# TODO: translate
play-magic-flute = You produce very attractive music.

# TODO: translate
play-magic-harp = You produce very attractive music.

# TODO: translate
play-music = You play some music.

# TODO: translate
play-nothing = You can't think of anything appropriate to play.

# TODO: translate
polymorph-controlled = What monster do you want to turn into?
# TODO: translate
polymorph-dismount = You can no longer ride your steed.

# TODO: translate
polymorph-newman-survive = You survive your attempted polymorph.

# TODO: translate
polymorph-revert = You return to your normal form.

# TODO: translate
polymorph-system-shock = Your body shudders and undergoes a violent transformation!

# TODO: translate
polymorph-system-shock-fatal = The system shock from the polymorph kills you!

# TODO: translate
potion-acid-resist = Your affinity to acid disappears!

# TODO: translate
potion-see-invisible-cursed = You thought you saw something.

# TODO: translate
potion-sickness-mild = Yecch! This stuff tastes like poison.

# TODO: translate
potion-uneasy = You feel uneasy.

# TODO: translate
pray-angry-curse = You feel that your possessions are less effective.

# TODO: translate
pray-angry-displeased = You feel that { $god } is displeased.

# TODO: translate
pray-angry-lose-wis = Your wisdom diminishes.

# TODO: translate
pray-angry-punished = You are punished for your misbehavior!

# TODO: translate
pray-angry-summon = { $god } summons hostile monsters!

# TODO: translate
pray-angry-zap = Suddenly, a bolt of lightning strikes you!

# TODO: translate
pray-bless-weapon = Your weapon softly glows.

# TODO: translate
pray-castle-tune = You hear a voice echo: "The passtune sounds like..."

# TODO: translate
pray-cross-altar-penalty = You have a strange forbidding feeling.

# TODO: translate
pray-demon-rejected = { $god } is not deterred...

# TODO: translate
pray-fix-trouble = { $god } fixes your trouble.

# TODO: translate
pray-gehennom-no-help = { $god } does not seem to be able to reach you in Gehennom.

# TODO: translate
pray-golden-glow = A golden glow surrounds you.

# TODO: translate
pray-grant-intrinsic = You feel the power of { $god }.

# TODO: translate
pray-grant-spell = Divine knowledge fills your mind!

# TODO: translate
pray-indifferent = { $god } seems indifferent.

# TODO: translate
pray-moloch-laughter = Moloch laughs at your prayers.

# TODO: translate
pray-pleased = You feel that { $god } is pleased.

# TODO: translate
pray-uncurse-all = You feel like someone is helping you.

# TODO: translate
pray-undead-rebuke = You feel unworthy.

# TODO: translate
priest-angry = The priest gets angry!

# TODO: translate
priest-calmed = The priest calms down.

# TODO: translate
priest-virtues-of-poverty = The priest preaches the virtues of poverty.

# TODO: translate
priest-wrong-alignment = The priest mutters disapprovingly.

# TODO: translate
punishment-applied = You are punished!

# TODO: translate
punishment-removed = You feel the iron ball disappear.

# TODO: translate
quest-assigned = Your quest has been assigned.
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

# TODO: translate
region-fog-obscures = A cloud of fog obscures your vision!

# TODO: translate
reveal-monsters-none = There are no monsters to reveal.

# TODO: translate
rub-lamp-djinni = You rub the lamp and a djinni emerges!

# TODO: translate
rub-lamp-nothing = Nothing happens.

# TODO: translate
rub-no-effect = Nothing seems to happen.

# TODO: translate
rub-touchstone = You rub against the touchstone.

# TODO: translate
rump-gets-wet = Your rump gets wet.

# TODO: translate
sacrifice-alignment-convert = You feel the power of { $god } over you.

# TODO: translate
sacrifice-altar-convert = The altar is converted!

# TODO: translate
sacrifice-altar-reject = { $god } rejects your sacrifice!

# TODO: translate
sacrifice-conversion-rejected = You hear a thunderclap!

# TODO: translate
sacrifice-nothing = Your sacrifice disappears!

# TODO: translate
sacrifice-unicorn-insult = { $god } finds your sacrifice insulting.

# TODO: translate
sanctum-be-gone = "Be gone, mortal!"

# TODO: translate
sanctum-desecrate = You desecrate the high altar!

# TODO: translate
sanctum-infidel = "You dare enter the sanctum, infidel!"

# TODO: translate
scroll-confuse-cure = You feel less confused.

# TODO: translate
scroll-confuse-self = You feel confused.

# TODO: translate
scroll-destroy-armor-disenchant = Your armor is less effective!

# TODO: translate
scroll-destroy-armor-id = Your armor glows then fades.

# TODO: translate
scroll-fire-confused = Your scroll erupts in flame!

# TODO: translate
scroll-genocide-reverse = You create a swarm of monsters!

# TODO: translate
scroll-genocide-reverse-self = You feel a change coming over you.

# TODO: translate
scroll-identify-self = You feel self-knowledgeable...

# TODO: translate
see-item-here = You see an item here.
# TODO: translate
see-items-here = You see items here.

# TODO: translate
scroll-cant-read-blind = You can't read while blind!

# TODO: translate
sick-deaths-door = You are at death's door.

# TODO: translate
sick-illness-severe = You feel deathly sick.

# TODO: translate
sit-already-riding = You are already riding something.

# TODO: translate
sit-in-water = You sit in the water.

# TODO: translate
sit-no-seats = There is nothing here to sit on.

# TODO: translate
sit-on-air = Having fun sitting on the air?

# TODO: translate
sit-on-altar = You sit on the altar.

# TODO: translate
sit-on-floor = Having fun sitting on the floor?

# TODO: translate
sit-on-grave = You sit on the headstone.

# TODO: translate
sit-on-ice = The ice feels cold.

# TODO: translate
sit-on-lava = The lava burns you!

# TODO: translate
sit-on-sink = You sit on the sink.

# TODO: translate
sit-on-stairs = You sit on the stairs.

# TODO: translate
sit-on-throne = You feel a strange sensation.

# TODO: translate
sit-tumble-in-place = You tumble in place.

# TODO: translate
sleepy-yawn = You yawn.

# TODO: translate
slime-burned-away = The slime is burned away!

# TODO: translate
sliming-become-slime = You have become a green slime!

# TODO: translate
sliming-limbs-oozy = Your limbs are getting oozy.

# TODO: translate
sliming-skin-peeling = Your skin begins to peel.

# TODO: translate
sliming-turning-green = You are turning a little green.

# TODO: translate
sliming-turning-into = You are turning into a green slime!

# TODO: translate
spell-aggravation = You feel as if something is very angry.

# TODO: translate
spell-book-full = You know too many spells already.

# TODO: translate
spell-cancellation-hit = { $target } is covered by a shimmering light!

# TODO: translate
spell-cancellation-miss = You miss { $target }.

# TODO: translate
spell-cast-fail = You fail to cast the spell correctly.

# TODO: translate
spell-cause-fear-none = No monsters are frightened.

# TODO: translate
spell-charm-monster-hit = { $target } is charmed!

# TODO: translate
spell-charm-monster-miss = { $target } resists!

# TODO: translate
spell-clairvoyance = You sense your surroundings.

# TODO: translate
spell-confuse-monster-hit = { $target } seems confused!

# TODO: translate
spell-confuse-monster-miss = { $target } resists!

# TODO: translate
spell-confuse-monster-touch = Your hands begin to glow red.

# TODO: translate
spell-create-familiar = A familiar creature appears!

# TODO: translate
spell-create-monster = A monster appears!

# TODO: translate
spell-cure-blindness-not-blind = You aren't blind.

# TODO: translate
spell-cure-sickness-not-sick = You aren't sick.

# TODO: translate
spell-curse-items = You feel as if you need an exorcist.

# TODO: translate
spell-destroy-armor = Your armor crumbles away!

# TODO: translate
spell-detect-monsters-none = You don't sense any monsters.

# TODO: translate
spell-detect-unseen-none = You don't sense any unseen things.

# TODO: translate
spell-dig-nothing = The digging ray has no effect here.

# TODO: translate
spell-drain-life-hit = { $target } suddenly seems weaker!

# TODO: translate
spell-drain-life-miss = You miss { $target }.

# TODO: translate
spell-finger-of-death-kill = { $target } dies!

# TODO: translate
spell-finger-of-death-resisted = { $target } resists!

# TODO: translate
spell-haste-self = You feel yourself moving more quickly.

# TODO: translate
spell-healing = You feel better.

# TODO: translate
spell-identify = You feel self-knowledgeable...

# TODO: translate
spell-insufficient-power = You don't have enough energy to cast that spell.

# TODO: translate
spell-invisibility = You feel rather airy.

# TODO: translate
spell-jumping = You jump!

# TODO: translate
spell-jumping-blocked = Something blocks your jump.

# TODO: translate
spell-knock = A door opens!

# TODO: translate
spell-light = A lit field surrounds you!

# TODO: translate
spell-magic-mapping = A map of your surroundings appears!

# TODO: translate
spell-need-direction = In what direction?

# TODO: translate
spell-no-spellbook = You don't have any spellbooks to study.

# TODO: translate
spell-polymorph-hit = { $target } undergoes a transformation!

# TODO: translate
spell-polymorph-miss = You miss { $target }.

# TODO: translate
spell-protection-disappears = Your golden glow fades.

# TODO: translate
spell-protection-less-dense = Your golden haze becomes less dense.

# TODO: translate
spell-remove-curse = You feel like someone is helping you.

# TODO: translate
spell-restore-ability-nothing = You feel momentarily refreshed.

# TODO: translate
spell-restore-ability-restored = Wow! This makes you feel great!

# TODO: translate
spell-sleep-hit = { $target } falls asleep!

# TODO: translate
spell-sleep-miss = { $target } resists!

# TODO: translate
spell-slow-monster-hit = { $target } seems to slow down.

# TODO: translate
spell-slow-monster-miss = { $target } resists!

# TODO: translate
spell-stone-to-flesh-cured = You feel limber!

# TODO: translate
spell-stone-to-flesh-nothing = Nothing happens.

# TODO: translate
spell-summon-insects = You summon insects!

# TODO: translate
spell-summon-monster = You summon a monster!

# TODO: translate
spell-teleport-away-hit = { $target } disappears!

# TODO: translate
spell-teleport-away-miss = { $target } resists!

# TODO: translate
spell-turn-undead-hit = { $target } flees!

# TODO: translate
spell-turn-undead-miss = { $target } resists!

# TODO: translate
spell-unknown = You don't know that spell.

# TODO: translate
spell-weaken = { $target } suddenly seems weaker!

# TODO: translate
spell-wizard-lock = A door locks shut!

# TODO: translate
stairs-at-top = You are at the top of the dungeon.

# TODO: translate
stairs-not-here = You don't see any stairs here.

# TODO: translate
status-blindness-end = You can see again.

# TODO: translate
status-confusion-end = You feel less confused now.

# TODO: translate
status-fall-asleep = You fall asleep.

# TODO: translate
status-fumble-trip = You trip over something.

# TODO: translate
status-fumbling-end = You feel less clumsy.

# TODO: translate
status-fumbling-start = You feel clumsy.

# TODO: translate
status-hallucination-end = Everything looks SO boring now.

# TODO: translate
status-invisibility-end = You are no longer invisible.

# TODO: translate
status-levitation-end = You float gently to the floor.

# TODO: translate
status-paralysis-end = You can move again.

# TODO: translate
status-paralyzed-cant-move = You can't move!

# TODO: translate
status-sick-cured = What a relief!

# TODO: translate
status-sick-recovered = You feel better.

# TODO: translate
status-sleepy-end = You feel awake.

# TODO: translate
status-sleepy-start = You feel drowsy.

# TODO: translate
status-speed-end = You feel yourself slow down.

# TODO: translate
status-stun-end = You feel less stunned now.

# TODO: translate
status-vomiting-end = You feel less nauseated now.

# TODO: translate
status-vomiting-start = You feel nauseated.

# TODO: translate
status-wounded-legs-healed = Your legs feel better.

# TODO: translate
status-wounded-legs-start = Your legs are in bad shape!

# TODO: translate
steal-item-from-you = { $monster } steals { $item }!

# TODO: translate
steal-no-gold = { $monster } finds no gold on you.

# TODO: translate
steal-nothing-to-take = { $monster } finds nothing to steal.

# TODO: translate
steed-stops-galloping = Your steed slows to a halt.
# TODO: translate
steed-swims = Your steed paddles through the water.

# TODO: translate
stoning-limbs-stiffening = Your limbs are stiffening.

# TODO: translate
stoning-limbs-stone = Your limbs have turned to stone.

# TODO: translate
stoning-slowing-down = You are slowing down.

# TODO: translate
stoning-turned-to-stone = You have turned to stone.

# TODO: translate
stoning-you-are-statue = You are a statue.

# TODO: translate
swap-no-secondary = You have no secondary weapon.

# TODO: translate
swap-success = You swap your weapons.

# TODO: translate
swap-welded = Your weapon is welded to your hand!

# TODO: translate
swim-lava-burn = You burn to a crisp in the lava!

# TODO: translate
swim-water-paddle = You paddle in the water.

# TODO: translate
temple-eerie = You have an eerie feeling...

# TODO: translate
temple-ghost-appears = A ghost appears before you!

# TODO: translate
temple-shiver = You shiver.

# TODO: translate
temple-watched = You feel as though someone is watching you.

# TODO: translate
throne-genocide = A voice echoes: "Thou shalt choose who lives and who dies!"

# TODO: translate
throne-identify = You feel self-knowledgeable...

# TODO: translate
throne-no-position = You can't sit there from this position.

# TODO: translate
throne-not-here = There is no throne here.

# TODO: translate
throne-nothing = Nothing seems to happen.

# TODO: translate
throne-vanishes = The throne vanishes in a puff of logic.

# TODO: translate
throne-wish = A voice echoes: "Thy wish is granted!"

# TODO: translate
tip-cannot-reach = You can't reach that.

# TODO: translate
tip-empty = That container is empty.

# TODO: translate
tip-locked = That container is locked.

# TODO: translate
tool-bell-cursed-summon = The bell summons hostile creatures!

# TODO: translate
tool-bell-cursed-undead = The bell summons undead!

# TODO: translate
tool-bell-no-sound = But the sound is muffled.

# TODO: translate
tool-bell-opens = Something opens...

# TODO: translate
tool-bell-reveal = Things open around you...

# TODO: translate
tool-bell-ring = The bell rings.

# TODO: translate
tool-bell-wake-nearby = The ringing wakes nearby monsters!

# TODO: translate
tool-bullwhip-crack = You crack the bullwhip!

# TODO: translate
tool-camera-no-target = There is nothing to photograph.

# TODO: translate
tool-candelabrum-extinguish = The candelabrum's candles are extinguished.

# TODO: translate
tool-candelabrum-no-candles = The candelabrum has no candles attached.

# TODO: translate
tool-candle-extinguish = You extinguish the candle.

# TODO: translate
tool-candle-light = You light the candle.

# TODO: translate
tool-cream-pie-face = You get cream pie on your face!

# TODO: translate
tool-dig-no-target = There is nothing to dig here.

# TODO: translate
tool-drum-earthquake = The entire dungeon is shaking around you!

# TODO: translate
tool-drum-no-charges = The drum is out of charges.

# TODO: translate
tool-drum-thump = You beat the drum.

# TODO: translate
tool-figurine-hostile = The figurine transforms into a hostile monster!

# TODO: translate
tool-figurine-peaceful = The figurine transforms into a peaceful monster.

# TODO: translate
tool-figurine-tame = The figurine transforms into a pet!

# TODO: translate
tool-grease-empty = The can of grease is empty.

# TODO: translate
tool-grease-hands = Your hands are too slippery to hold anything!

# TODO: translate
tool-grease-slip = Your greased { $item } slips off!

# TODO: translate
tool-horn-no-charges = The horn is out of charges.

# TODO: translate
tool-horn-toot = You produce a frightful, horrible sound.

# TODO: translate
tool-leash-no-pet = There is no pet nearby to leash.

# TODO: translate
tool-lockpick-breaks = Your lockpick breaks!

# TODO: translate
tool-magic-lamp-djinni = A djinni emerges from the lamp!

# TODO: translate
tool-magic-whistle = You produce a strange whistling sound.

# TODO: translate
tool-mirror-self = You look ugly in the mirror.

# TODO: translate
tool-no-locked-door = There is no locked door there.

# TODO: translate
tool-nothing-happens = Nothing seems to happen.

# TODO: translate
tool-polearm-no-target = There is nothing to hit there.

# TODO: translate
tool-saddle-no-mount = There is nothing to put a saddle on.

# TODO: translate
tool-tin-whistle = You produce a high whistling sound.

# TODO: translate
tool-tinning-no-corpse = There is no corpse to tin here.

# TODO: translate
tool-touchstone-identify = You identify the gems by rubbing them on the touchstone.

# TODO: translate
tool-touchstone-shatter = The gem shatters!

# TODO: translate
tool-touchstone-streak = The gem leaves a streak on the touchstone.

# TODO: translate
tool-towel-cursed-gunk = Your face is covered with gunk!

# TODO: translate
tool-towel-cursed-nothing = You can't get the gunk off!

# TODO: translate
tool-towel-cursed-slimy = Your face feels slimy.

# TODO: translate
tool-towel-nothing = Your face is already clean.

# TODO: translate
tool-towel-wipe-face = You've got the glop off.

# TODO: translate
tool-unihorn-cured = You feel better!

# TODO: translate
tool-unihorn-cursed = The unicorn horn is cursed!

# TODO: translate
tool-unihorn-nothing = Nothing seems to happen.

# TODO: translate
tool-unlock-fail = You fail to unlock it.

# TODO: translate
tool-unlock-success = You unlock it.

# TODO: translate
tool-whistle-no-pets = You don't have any pets nearby.

# TODO: translate
tunnel-blocked = There is no room to tunnel here.

# TODO: translate
turn-no-undead = There are no undead to turn.

# TODO: translate
turn-not-clerical = You don't know how to turn undead.

# TODO: translate
untrap-failed = You fail to disarm the trap.

# TODO: translate
untrap-no-trap = You don't find any traps.

# TODO: translate
untrap-success = You disarm the trap!

# TODO: translate
untrap-triggered = You triggered the trap!

# TODO: translate
vanquished-none = No monsters have been vanquished yet.

# TODO: translate
vault-guard-disappear = The guard disappears.

# TODO: translate
vault-guard-escort = The guard escorts you out.

# TODO: translate
vault-guard-state-change = The guard changes stance.

# TODO: translate
vomiting-about-to = You are about to vomit.

# TODO: translate
vomiting-cant-think = You can't think straight.

# TODO: translate
vomiting-incredibly-sick = You feel incredibly sick.

# TODO: translate
vomiting-mildly-nauseated = You feel mildly nauseated.

# TODO: translate
vomiting-slightly-confused = You feel slightly confused.

# TODO: translate
vomiting-vomit = You vomit!

# TODO: translate
wait = Time passes...

# TODO: translate
wand-cancel-monster = { $target } is covered by a shimmering light!

# TODO: translate
wand-digging-miss = The digging beam misses.

# TODO: translate
wipe-cream-off = You wipe the cream off your face.

# TODO: translate
wipe-cursed-towel = The towel is cursed!

# TODO: translate
wipe-nothing = There is nothing to wipe off.

# TODO: translate
wizard-curse-items = You feel as if you need an exorcist.

# TODO: translate
wizard-detect-all = You sense everything around you.

# TODO: translate
wizard-detect-monsters = You feel as if something is watching you.

# TODO: translate
wizard-detect-objects = You sense the presence of objects.

# TODO: translate
wizard-detect-traps = You feel warned about nearby traps.

# TODO: translate
wizard-double-trouble = "Double Trouble..."

# TODO: translate
wizard-identify-all = You feel self-knowledgeable...

# TODO: translate
wizard-genesis = A { $monster } appears beside you.

# TODO: translate
wizard-genesis-failed = Nothing answers your request for { $monster }.

# TODO: translate
wizard-kill = You wipe out { $count } monster(s) on this level.

# TODO: translate
wizard-kill-none = There are no monsters here to wipe out.

# TODO: translate
wizard-map-revealed = An image of your surroundings forms in your mind!

# TODO: translate
wizard-respawned = The Wizard of Yendor rises again!

# TODO: translate
wizard-steal-amulet = The Wizard of Yendor steals the Amulet!

# TODO: translate
wizard-summon-nasties = New nasties appear from thin air!

# TODO: translate
wizard-where-current = You are on { $location } (absolute depth { $absolute }) at { $x },{ $y }.

# TODO: translate
wizard-where-special = { $level } lies on { $location }.

# TODO: translate
wizard-wish = Your wish is granted: { $item }.

# TODO: translate
wizard-wish-adjusted = Your wish is adjusted to: { $item }.

# TODO: translate
wizard-wish-adjusted-floor = Your wish is adjusted: { $item } drops at your feet.

# TODO: translate
wizard-wish-failed = Nothing answers your wish for "{ $wish }".

# TODO: translate
wizard-wish-floor = Your wish is granted: { $item } drops at your feet.

# TODO: translate
worm-grows = The long worm grows longer!

# TODO: translate
worm-shrinks = The long worm shrinks!

# TODO: translate
worn-gauntlets-power-off = You feel weaker.

# TODO: translate
worn-gauntlets-power-on = You feel stronger!

# TODO: translate
worn-helm-brilliance-off = You feel ordinary.

# TODO: translate
worn-helm-brilliance-on = You feel brilliant!

# TODO: translate
write-no-marker = You don't have a magic marker.

# TODO: translate
write-not-enough-charges = Your marker is too dry to write that!

# TODO: translate
write-scroll-fail-daiyen-fansen = Your marker dries out!

# TODO: translate
write-spellbook-fail = The spellbook warps strangely, then turns blank.

# TODO: translate
write-spellbook-success = You successfully write the spellbook!
