## NetHack Babel — 繁體中文訊息目錄
## Fluent (.ftl) 格式 — https://projectfluent.org/

## ============================================================================
## 戰鬥 — 近戰
## ============================================================================

melee-hit-bare = { $attacker }擊中了{ $defender }！
melee-hit-weapon = { $attacker }用{ $weapon }擊中了{ $defender }！
melee-hit-slash = { $attacker }砍中了{ $defender }！
melee-hit-stab = { $attacker }刺中了{ $defender }！
melee-hit-bash = { $attacker }錘擊了{ $defender }！
melee-hit-whip = { $attacker }鞭打了{ $defender }！
melee-hit-bite = { $attacker }咬了{ $defender }！
melee-hit-claw = { $attacker }抓傷了{ $defender }！
melee-hit-sting = { $attacker }螫了{ $defender }！
melee-hit-butt = { $attacker }頂撞了{ $defender }！
melee-hit-kick = { $attacker }踢中了{ $defender }！
melee-miss = { $attacker }沒有打中{ $defender }。
melee-miss-barely = { $attacker }差一點就打中{ $defender }了。
critical-hit = 暴擊！
backstab = { $attacker }偷襲了{ $defender }！
joust-hit = { $attacker }用長矛衝刺了{ $defender }！
joust-lance-breaks = 你的長矛在衝擊中碎裂了！
attack-blocked = { $defender }擋住了攻擊。
attack-parried = { $defender }格開了攻擊。

## ============================================================================
## 戰鬥 — 遠程
## ============================================================================

ranged-hit = { $attacker }用{ $projectile }射中了{ $defender }！
ranged-miss = { $projectile }沒有射中{ $defender }。
ranged-miss-wide = { $projectile }從{ $defender }身旁飛過。
throw-hit = { $projectile }擊中了{ $defender }！
throw-miss = { $projectile }沒有打中{ $defender }，落在了地上。
spell-hit = { $spell }命中了{ $defender }！
spell-miss = { $spell }沒有命中{ $defender }。
spell-fizzle = 法術失效了。
wand-zap = 你揮動了{ $wand }！
wand-nothing = 什麼也沒有發生。
wand-wrested = 你從{ $wand }中榨出了最後一點能量。

## ============================================================================
## 戰鬥 — 傷害描述
## ============================================================================

damage-barely = 攻擊幾乎沒有傷到{ $defender }。
damage-light = { $defender }受了輕傷。
damage-moderate = { $defender }受了中等程度的傷。
damage-heavy = { $defender }受了重傷。
damage-severe = { $defender }傷勢嚴重。
damage-critical = { $defender }傷勢危急！

## ============================================================================
## 戰鬥 — 被動傷害
## ============================================================================

passive-acid = 酸液灼燒著你！
passive-fire = 你被燒傷了！
passive-cold = 你被凍傷了！
passive-shock = 你被電擊了！
passive-poison = 你感到噁心！
passive-drain = 你感到精力被吸走了！
passive-corrode = 你的{ $item }被腐蝕了！
passive-stun = 你被打得頭昏眼花！
passive-slow = 你感到自己慢了下來！
passive-paralyze = 你被麻痺了！

## ============================================================================
## 戰鬥 — 死亡訊息
## ============================================================================

entity-killed = { $entity }被殺死了！
entity-destroyed = { $entity }被摧毀了！
entity-dissolved = { $entity }溶解了！
entity-evaporates = { $entity }蒸發了！
entity-turns-to-dust = { $entity }化為了灰燼！
you = 你
you-hit-monster = 你擊中了{ $monster }！
you-miss-monster = 你沒有打中{ $monster }。
you-kill-monster = 你殺死了{ $monster }！
you-destroy-monster = 你摧毀了{ $monster }！
you-dissolve-monster = { $monster }溶解了！
monster-hits-you = { $monster }擊中了你！
monster-misses-you = { $monster }沒有打中你。
monster-kills-you = { $monster }殺死了你！
monster-turns-to-stone = { $monster }變成了石頭！
monster-flees = { $monster }轉身逃跑了！
monster-falls-asleep = { $monster }睡著了。

## ============================================================================
## 移動 — 門
## ============================================================================

door-opened = 門開了。
door-closed = 門關上了。
door-locked = 這扇門鎖著。
door-broken = 你破開了門！
door-unlock = 你打開了門鎖。
door-lock = 你鎖上了門。
door-kick = 你踢了門一腳！
door-kick-fail = 砰！！！門紋絲不動。
door-resist = 門沒有被打開！
door-jammed = 門卡住了。

## ============================================================================
## 移動 — 碰撞和地形
## ============================================================================

bump-wall = 你撞到了牆上。
bump-boulder = 你推動了巨石。
bump-boulder-fail = 巨石紋絲不動。
bump-closed-door = 哎喲！你撞到了一扇關著的門上。
bump-monster = 你撞上了{ $monster }。
swim-lava = 你正在熔岩中游泳！
swim-water = 你掉進了水裡！
swim-sink = 你沉入了水面之下！
terrain-ice = 冰面很滑！
terrain-ice-slip = 你在冰上滑倒了！
terrain-mud = 你陷入了泥沼。

## ============================================================================
## 移動 — 陷阱
## ============================================================================

trap-triggered = 你觸發了{ $trap }！
trap-disarmed = 你拆除了{ $trap }。
trap-pit-fall = 你掉進了一個坑裡！
trap-spiked-pit = 你掉進了一個佈滿尖刺的坑裡！
trap-arrow = 一支箭向你射來！
trap-dart = 一支小飛鏢向你射來！
trap-bear = 你踩到了捕獸夾！
trap-teleport = 你感到一陣扭曲！
trap-level-teleport = 你感到一陣劇烈的變化！
trap-magic-portal = 你感到一陣眩暈。
trap-fire = 一根火柱噴發了！
trap-rolling-boulder = 一塊巨石向你滾來！
trap-squeaky-board = 你腳下的木板發出吱嘎聲。
trap-web = 你被蛛網纏住了！
trap-rust = 一股水流噴向你！
trap-polymorph = 你感到身體正在發生變化！

## ============================================================================
## 移動 — 樓梯和層級變化
## ============================================================================

stairs-up = 你走上樓梯。
stairs-down = 你走下樓梯。
stairs-nothing-up = 你沒法在這裡往上走。
stairs-nothing-down = 你沒法在這裡往下走。
level-change = 你進入了{ $level }。
level-feeling = 你對這一層有{ $feeling ->
    [good] 好的
    [bad] 不好的
   *[neutral] 不確定的
}預感。
level-feeling-objects = 你感覺這一層有貴重物品。
level-enter-shop = 你進入了{ $shopkeeper }的{ $shoptype }。
level-leave-shop = 你離開了商店。
elbereth-engrave = 你在地上刻下了「Elbereth」。
elbereth-warn = { $monster }看起來很害怕！
elbereth-fade = 刻文褪色了。

## ============================================================================
## 移動 — 環境互動
## ============================================================================

fountain-drink = 你喝了噴泉的水。
fountain-dry = 噴泉乾涸了！
fountain-wish = 你看到一個閃光的水池。
sink-drink = 你喝了水龍頭的水。
sink-ring = 你聽到一枚戒指沿著排水管滾下去的聲音。
altar-pray = 你開始向{ $god }祈禱。
altar-sacrifice = 你向{ $god }獻上了{ $corpse }。
altar-desecrate = 你感到一股黑暗的力量。
throne-sit = 你坐在了王座上。
grave-dig = 你挖開了墳墓。
grave-disturb = 你打擾了{ $entity }的安息。

## ============================================================================
## 狀態 — 飢餓
## ============================================================================

hunger-satiated = 你吃飽了。
hunger-not-hungry = 你不餓。
hunger-hungry = 你餓了。
hunger-weak = 你感到虛弱。
hunger-fainting = 你因為飢餓而昏倒了。
hunger-starved = 你餓死了。

## ============================================================================
## 狀態 — 生命值和等級
## ============================================================================

hp-gained = 你感覺好了{ $amount ->
    [one] 一點
   *[other] 許多
}。
hp-lost = 你感覺更糟了{ $amount ->
    [one] 一點
   *[other] 許多
}。
hp-full = 你感覺完全恢復了。
hp-restored = 你的生命值恢復了。
level-up = 歡迎來到經驗等級{ $level }！
level-down = 你感覺經驗不如以前了。
level-max = 你感到無比強大！

## ============================================================================
## 狀態 — 屬性獲得/失去
## ============================================================================

speed-gain = 你感到腳步輕快！
speed-lose = 你慢了下來。
speed-very-fast = 你感到速度飛快！
strength-gain = 你感到力大無窮！
strength-lose = 你感到力量減退！
telepathy-gain = 你感到一種奇異的心靈感應。
telepathy-lose = 你的感覺恢復了正常。
invisibility-gain = 你感到自己變得透明了！
invisibility-lose = 你重新變得可見了。
see-invisible-gain = 你感到洞察力增強了！
see-invisible-lose = 你的洞察力減弱了。
stealth-gain = 你感到自己的腳步更輕了！
stealth-lose = 你感到自己變得笨拙了。
fire-resist-gain = 你感到一陣涼意。
fire-resist-lose = 你感到溫暖了。
cold-resist-gain = 你感到溫暖。
cold-resist-lose = 你感到寒冷。
shock-resist-gain = 你感到自己被絕緣了。
shock-resist-lose = 你感到自己容易導電了。
poison-resist-gain = 你感到身體健康。
poison-resist-lose = 你感到身體不如以前健康了。

## ============================================================================
## 狀態 — 異常狀態
## ============================================================================

poison-affected = 你感覺{ $severity ->
    [mild] 有點不舒服
    [moderate] 生病了
   *[severe] 病得很重
}。
confusion-start = 你感到頭暈目眩。
confusion-end = 你不再那麼暈了。
blindness-start = 你什麼也看不見了！
blindness-end = 你又能看見了。
stun-start = 你搖搖晃晃……
stun-end = 你感到站穩了一些。
hallucination-start = 哇喔！一切看起來都好迷幻！
hallucination-end = 你恢復了正常。
sleep-start = 你感到昏昏欲睡……
sleep-end = 你醒了過來。
petrification-start = 你正在變成石頭！
petrification-cure = 你感到身體靈活了一些。
lycanthropy-start = 你感到發燒了。
lycanthropy-cure = 你感覺好多了。
levitation-start = 你飄了起來！
levitation-end = 你緩緩降落。

## ============================================================================
## 狀態 — 負重
## ============================================================================

encumbrance-unencumbered = 你行動自如。
encumbrance-burdened = 你負擔較重。
encumbrance-stressed = 你負擔很重。
encumbrance-strained = 你已經不堪重負！
encumbrance-overtaxed = 你已經超負荷了！
encumbrance-overloaded = 你負重過大，無法移動！

## ============================================================================
## 物品 — 拾取和丟棄
## ============================================================================

item-picked-up = { $actor }撿起了{ $item }。
item-dropped = { $actor }放下了{ $item }。
you-pick-up = 你撿起了{ $item }。
you-drop = 你放下了{ $item }。
you-pick-up-gold = 你撿起了{ $amount }枚金幣。
nothing-to-pick-up = 這裡沒有東西可以撿。
too-many-items = 你的東西太多了！

## ============================================================================
## 物品 — 揮舞武器
## ============================================================================

item-wielded = { $actor }裝備了{ $item }。
you-wield = 你揮舞起{ $weapon }。
    .two-handed = （雙手武器）
you-wield-already = 你已經在使用那件武器了！
you-unwield = 你收起了{ $weapon }。
you-wield-nothing = 你空著雙手。
weapon-weld-cursed = { $weapon }黏在了你的手上！

## ============================================================================
## 物品 — 護甲
## ============================================================================

item-worn = { $actor }穿上了{ $item }。
item-removed = { $actor }脫下了{ $item }。
you-wear = 你穿上了{ $armor }。
you-remove = 你脫下了{ $armor }。
you-remove-cursed = 你脫不下{ $armor }。它被詛咒了！
armor-crumbles = 你的{ $armor }碎裂成灰了！

## ============================================================================
## 物品 — 鑑定和狀態
## ============================================================================

item-identified = 你知道了{ $item }就是{ $identity }。
item-damaged = { $item }受損了！
item-destroyed = { $item }被摧毀了！
item-cursed = { $item }被詛咒了！
item-blessed = { $item }被祝福了。
item-enchanted = { $item }閃了一下{ $color }的光。
item-rusted = { $item }生鏽了。
item-burnt = { $item }被燒焦了！
item-rotted = { $item }腐爛了。
item-corroded = { $item }被腐蝕了。
item-eroded-away = { $item }完全腐蝕掉了！

## ============================================================================
## 物品 — 食物和飲食
## ============================================================================

eat-start = 你開始吃{ $food }。
eat-finish = 你吃完了{ $food }。
eat-delicious = 真好吃！
eat-disgusting = 嘔！真難吃！
eat-poisoned = 呸——一定是有毒的！
eat-rotten = 呃——這食物腐爛了！
eat-cannibal = 你這個食人者！你感到致命的噁心。
eat-corpse-old = 這具{ $monster }的屍體有點不新鮮了。

## ============================================================================
## 物品 — 藥水和卷軸
## ============================================================================

potion-drink = 你喝了{ $potion }。
potion-shatter = { $potion }碎了！
potion-boil = { $potion }沸騰蒸發了。
potion-freeze = { $potion }結冰碎裂了！
scroll-read = 你讀卷軸的時候，它消失了。
scroll-blank = 這張卷軸似乎是空白的。
scroll-cant-read = 你看不見，沒法閱讀！
spellbook-read = 你開始研讀{ $spellbook }。
spellbook-learn = 你學會了{ $spell }法術！
spellbook-forget = 你忘記了{ $spell }法術。
spellbook-too-hard = 這本魔法書對你來說太難了。

## ============================================================================
## 物品 — 金幣
## ============================================================================

gold-pick-up = 你撿起了{ $amount }枚金幣。
gold-drop = 你丟下了{ $amount }枚金幣。
gold-paid = 你支付了{ $amount }枚金幣。
gold-received = 你收到了{ $amount }枚金幣。

## ============================================================================
## 物品 — 容器
## ============================================================================

container-open = 你打開了{ $container }。
container-close = 你關上了{ $container }。
container-empty = { $container }是空的。
container-locked = { $container }鎖著。
container-trap = 你觸發了{ $container }上的陷阱！
container-looted = 你搜索了{ $container }。

## ============================================================================
## 怪物 — 行動
## ============================================================================

monster-moves = { $monster }移動了。
monster-picks-up = { $monster }撿起了{ $item }。
monster-wields = { $monster }揮舞起{ $item }！
monster-wears = { $monster }穿上了{ $item }。
monster-eats = { $monster }吃了{ $food }。
monster-drinks = { $monster }喝了{ $potion }！
monster-casts = { $monster }施放了法術！
monster-breathes = { $monster }噴出了{ $element }！
monster-summons = { $monster }召喚了援軍！
monster-steals = { $monster }偷走了{ $item }！
monster-grabs = { $monster }抓住了你！
monster-throws = { $monster }投擲了{ $item }！
monster-zaps = { $monster }揮動了{ $wand }！

## ============================================================================
## 怪物 — 聲音
## ============================================================================

sound-growl = 你聽到一陣低沉的咆哮。
sound-roar = 你聽到一聲怒吼！
sound-hiss = 你聽到嘶嘶聲！
sound-buzz = 你聽到嗡嗡聲。
sound-chug = 你聽到咕咚咕咚的聲音。
sound-splash = 你聽到水花聲。
sound-clank = 你聽到鏗鏘聲。
sound-scream = 你聽到一聲慘叫！
sound-squeak = 你聽到吱吱聲。
sound-laugh = 你聽到瘋狂的笑聲！
sound-wail = 你聽到哀嚎聲。
sound-whisper = 你聽到竊竊私語。
sound-coins = 你聽到金幣叮噹作響。
sound-footsteps = 你聽到腳步聲。
sound-digging = 你聽到挖掘聲。

## ============================================================================
## 怪物 — 寵物訊息
## ============================================================================

pet-eats = 你的{ $pet }吃了{ $food }。
pet-drops = 你的{ $pet }放下了{ $item }。
pet-picks-up = 你的{ $pet }撿起了{ $item }。
pet-whimper = 你的{ $pet }嗚咽著。
pet-happy = 你的{ $pet }看起來很高興。
pet-wag = 你的{ $pet }搖了搖尾巴。
pet-hostile = 你的{ $pet }變得狂暴了！
pet-tame = 你馴服了{ $monster }。
pet-name = 你想給你的{ $pet }起什麼名字？

## ============================================================================
## 介面 — 提示
## ============================================================================

more-prompt = ——繼續——
quit-prompt = 你確定要退出嗎？
really-quit = 真的要退出嗎？
prompt-direction = 朝哪個方向？
prompt-eat = 你想吃什麼？
prompt-drink = 你想喝什麼？
prompt-read = 你想閱讀什麼？
prompt-zap = 你想使用哪根魔杖？
prompt-throw = 你想投擲什麼？
prompt-name = 你想命名什麼？
prompt-call = 稱之為：
prompt-confirm = 你確定嗎？[yn]
prompt-pay = 花{ $amount }購買{ $item }？

## ============================================================================
## 介面 — 遊戲結束和得分
## ============================================================================

game-over = 你死了。分數：{ $score }。
game-over-escaped = 你逃出了地牢！
game-over-ascended = 你飛升成為了半神！
game-over-quit = 你退出了遊戲。
game-over-possessions = 你想鑑定你的物品嗎？
game-over-topten = 你進入了前十名！
game-over-score-final = 最終分數：{ $score }。
game-over-turns = 你堅持了{ $turns }個回合。
game-over-killer = 死因：{ $killer }。
game-over-epitaph = 安息吧，{ $name }。
gender-male = 男
gender-female = 女
gender-neuter = 無性
death-cause-killed-by = 被{ $killer }殺死
death-cause-starvation = 餓死
death-cause-poisoning = 死於中毒
death-cause-petrification = 變成了石頭
death-cause-drowning = 溺死
death-cause-burning = 被燒死
death-cause-disintegration = 被裂解了
death-cause-sickness = 病死
death-cause-strangulation = 被勒死
death-cause-falling = 摔死
death-cause-crushed-boulder = 被巨石壓死
death-cause-quit = 主動退出
death-cause-escaped = 成功逃脫
death-cause-ascended = 成功飛升
death-cause-trickery = 死於詭計
ui-tombstone-epitaph = { $name }，{ $level }級冒險者
ui-tombstone-info = { $cause } | 分數：{ $score } | 回合：{ $turns } | HP：{ $hp }/{ $maxhp }

## ============================================================================
## 介面 — 歡迎和狀態
## ============================================================================

welcome = 歡迎來到 NetHack Babel，{ $role }{ $name }！
welcome-back = 歡迎回到 NetHack Babel，{ $role }{ $name }！
character-description = { $race }{ $role }{ $name }
dungeon-level = 地牢第{ $depth }層
status-line = 生命:{ $hp }/{ $maxhp } 魔力:{ $pw }/{ $maxpw } 防禦:{ $ac } 經驗:{ $level }

## ============================================================================
## 介面 — 幫助
## ============================================================================

help-title = NetHack Babel 幫助
help-move = 使用 hjklyubn 或方向鍵移動。
help-attack = 走向怪物即可攻擊。
help-wait = 按 . 或 s 等待一回合。
help-search = 按 s 搜索隱藏的東西。
help-inventory = 按 i 查看背包。
help-pickup = 按 , 拾取物品。
help-drop = 按 d 丟棄物品。
help-stairs-up = 按 < 上樓。
help-stairs-down = 按 > 下樓。
help-eat = 按 e 進食。
help-quaff = 按 q 喝藥水。
help-read = 按 r 閱讀卷軸或魔法書。
help-wield = 按 w 揮舞武器。
help-wear = 按 W 穿戴護甲。
help-remove = 按 T 脫下護甲。
help-zap = 按 z 使用魔杖。

# 幫助 — 移動圖示
help-move-diagram =
    {"  "}y k u     左上 上 右上
    {"  "}h . l      左  .  右
    {"  "}b j n     左下 下 右下

# 幫助 — 符號
help-symbols-title = 符號說明：
help-symbol-player = @  = 你（玩家）
help-symbol-floor = .  = 地面
help-symbol-corridor = #  = 走廊
help-symbol-door-closed = +  = 關閉的門
help-symbol-door-open = |  = 打開的門
help-symbol-stairs-up = <  = 上樓梯
help-symbol-stairs-down = >  = 下樓梯
help-symbol-water = {"}"}  = 水/岩漿
help-symbol-fountain = {"{"} = 噴泉

# 幫助 — 附加命令
help-options = 按 O 打開設定。
help-look = 按 : 查看地面。
help-history = 按 Ctrl+P 查看訊息歷史。
help-shift-run = Shift+方向鍵 = 沿方向奔跑。
help-arrows = 方向鍵也可用於移動。

## ============================================================================
## 系統 — 存檔和讀檔
## ============================================================================

save-game = 正在儲存遊戲……
save-complete = 遊戲已儲存。
save-failed = 儲存失敗！
load-game = 正在恢復存檔……
load-complete = 遊戲已恢復。
load-failed = 無法恢復存檔。
load-version-mismatch = 存檔版本不匹配。

## ============================================================================
## 系統 — 設定
## ============================================================================

config-loaded = 設定已載入。
config-error = 設定錯誤：{ $message }。
config-option-set = 選項「{ $option }」已設定為「{ $value }」。
config-option-unknown = 未知選項：{ $option }。
config-language-set = 語言已設定為{ $language }。
config-language-unknown = 未知語言：{ $language }。

## ============================================================================
## 系統 — 錯誤訊息
## ============================================================================

error-generic = 出了點問題。
error-save-corrupt = 存檔已損壞。
error-out-of-memory = 記憶體不足！
error-file-not-found = 找不到檔案：{ $file }。
error-permission = 權限被拒絕。
error-panic = 嚴重錯誤：{ $message }
error-impossible = 程式異常：{ $message }

## ============================================================================
## 其他 — 查看和搜索
## ============================================================================

nothing-here = 這裡什麼都沒有。
something-here = 你看到這裡有{ $item }。
several-things = 這裡有好幾樣東西。
look-position = 你看到了{ $description }。
search-nothing = 你什麼也沒找到。
search-found = 你找到了{ $item }！
search-secret-door = 你發現了一扇暗門！
search-secret-passage = 你發現了一條暗道！
search-trap = 你發現了{ $trap }！

## ============================================================================
## 其他 — 負重和觸及
## ============================================================================

cannot-reach = 你夠不到那個。
too-heavy = { $item }太重了。
inventory-full = 你的背包滿了。
pay-prompt = 花{ $amount }購買{ $item }？
cannot-do-while-blind = 你看不見，沒法那樣做。
cannot-do-while-confused = 你太暈了。
cannot-do-while-stunned = 你太眩暈了。
cannot-do-while-hallu = 你現在無法集中精神。

## ============================================================================
## 物品 — 卷軸（閱讀效果）
## ============================================================================

scroll-dust = 你閱讀的時候，卷軸化為了灰燼。
scroll-enchant-weapon = 你的{ $weapon }閃了一下{ $color }的光。
scroll-enchant-armor = 你的{ $armor }閃了一下{ $color }的光。
scroll-identify = 你感到更有見識了。
scroll-identify-prompt = 你想鑑定什麼？
scroll-identify-all = 你鑑定了背包裡的所有物品！
scroll-remove-curse = 你感到有人在幫助你。
scroll-remove-curse-nothing = 你沒有感到任何不同。
scroll-teleport = 你感到一陣扭曲。
scroll-teleport-no-effect = 你感到短暫的迷失。
scroll-create-monster = 你聽到低沉的嗡嗡聲。
scroll-scare-monster = 你聽到遠處傳來瘋狂的笑聲。
scroll-confuse-monster = 你的雙手開始發出{ $color }的光。
scroll-magic-mapping = 你看到了地牢的全貌！
scroll-fire = 卷軸爆發出一根火柱！
scroll-earth = 大地在你腳下震動！
scroll-amnesia = 你感覺有些東西被遺忘了。
scroll-punishment = 你因為不良行為被懲罰了！
scroll-stinking-cloud = 一團惡臭的雲霧從卷軸中湧出。
scroll-charging = 你感到一股魔力湧入。
scroll-genocide = 一個雷鳴般的聲音在洞穴中迴盪！
scroll-light = 一道光芒充滿了房間！
scroll-food-detection = 你感覺到食物的存在。
scroll-gold-detection = 你感覺到金子的存在。
scroll-destroy-armor = 你的{ $armor }碎裂成灰了！
scroll-taming = 你感到魅力非凡。
scroll-mail = 你收到了一張信件卷軸。
scroll-blank-paper = 這張卷軸看起來是空白的。

## ============================================================================
## 物品 — 魔杖（使用效果）
## ============================================================================

wand-fire = 一道火焰從{ $wand }中射出！
wand-cold = 一道寒氣從{ $wand }中射出！
wand-lightning = 一道閃電從{ $wand }中射出！
wand-magic-missile = 一顆魔法飛彈從{ $wand }中射出！
wand-sleep = 一道催眠射線從{ $wand }中射出。
wand-death = 一道死亡射線從{ $wand }中射出！
wand-polymorph = 一道閃爍的射線從{ $wand }中射出。
wand-striking = 一道光束從{ $wand }中射出！
wand-slow = 一道減速射線從{ $wand }中射出。
wand-speed = 一道加速光束從{ $wand }中射出。
wand-undead-turning = 一道驅逐亡靈的光束從{ $wand }中射出。
wand-opening = 一道開啟光束從{ $wand }中射出。
wand-locking = 一道鎖定光束從{ $wand }中射出。
wand-probing = 一道探測射線從{ $wand }中射出。
wand-digging = 一道挖掘光束從{ $wand }中射出！
wand-teleportation = 一道傳送光束從{ $wand }中射出。
wand-create-monster = 你聽到低沉的嗡嗡聲。
wand-cancellation = 一道消除光束從{ $wand }中射出！
wand-make-invisible = 一道隱形光束從{ $wand }中射出。
wand-light = 一陣光芒從{ $wand }中湧出！
wand-darkness = 一團黑暗從{ $wand }中湧出。
wand-wishing = 你可以許一個願望。
wand-ray-reflect = 光束從{ $surface }上反射了！
wand-ray-bounce = 光束從牆上彈開了！
wand-ray-absorb = { $entity }吸收了射線。
wand-break = { $wand }碎裂並爆炸了！
wand-break-effect = 一團{ $element }湧了出來！
wand-charge-empty = { $wand }似乎沒有剩餘能量了。
wand-recharge = { $wand }閃了一下光。
wand-recharge-fail = { $wand }劇烈震動後爆炸了！
wand-turn-to-dust = { $wand }化為了灰燼。

## ============================================================================
## 物品 — 藥水（飲用效果）
## ============================================================================

potion-healing = 你感覺好了一些。
potion-extra-healing = 你感覺好多了！
potion-full-healing = 你感覺完全恢復了！
potion-gain-ability = 你感到自己的{ $stat ->
    [str] 力量
    [dex] 敏捷
    [con] 體質
    [int] 智力
    [wis] 智慧
   *[cha] 魅力
}增強了！
potion-gain-level = 你升了上去，穿過了天花板！
potion-gain-energy = 魔法能量在你體內流動！
potion-speed = 你感到速度飛快！
potion-invisibility = 你感到自己變得透明了！
potion-see-invisible = 你感到洞察力增強了！
potion-levitation = 你開始漂浮在空中！
potion-confusion = 哈？什麼？我在哪？
potion-blindness = 一切都變暗了！
potion-hallucination = 哇喔！一切看起來都好迷幻！
potion-sleeping = 你感到非常睏倦。
potion-paralysis = 你動不了了！
potion-poison = 你感到非常不舒服。
potion-acid = 酸液灼燒著你！
potion-oil = 那真順滑。
potion-water = 這嚐起來像水。
potion-holy-water = 你感到被淨化了。
potion-unholy-water = 你感到一股邪惡的氣息包圍著你。
potion-object-detection = 你感覺到物品的存在。
potion-monster-detection = 你感覺到怪物的存在。
potion-sickness = 你嘔吐了。
potion-restore-ability = 你感到力量恢復了！
potion-polymorph = 你感到身體正在發生變化。
potion-booze = 呃！這嚐起來像{ $liquid }！
potion-fruit-juice = 這嚐起來像{ $fruit }汁。
potion-mix = 藥水混合後產生了{ $result }。
potion-dilute = { $potion }變稀了。
potion-vapor = 你吸入了一股{ $effect }氣息。

## ============================================================================
## 物品 — 物品互動訊息
## ============================================================================

pickup-with-quantity = 你撿起了{ $quantity }個{ $item }。
drop-with-quantity = 你放下了{ $quantity }個{ $item }。
cannot-carry-more = 你沒法再拿更多了。
knapsack-full = 你的背包裝不下更多東西了。
encumbrance-prevents = 你負重太大，沒法做那個。
encumbrance-warning-burdened = 負重拖慢了你的腳步。
encumbrance-warning-stressed = 你在沉重的負擔下步履蹣跚。
encumbrance-warning-strained = 你幾乎無法在這樣的負重下移動！
encumbrance-warning-overtaxed = 你就要在負重下倒下了！
identify-result = { $item }是{ $identity }。
identify-already-known = 你已經知道了。
altar-buc-blessed = { $item }發出明亮的琥珀色光芒！
altar-buc-uncursed = { $item }發出微弱的琥珀色光芒。
altar-buc-cursed = { $item }籠罩著黑色的光暈。
altar-buc-unknown = 似乎什麼也沒有發生。
item-name-prompt = 你想給{ $item }起什麼名字？
item-called-prompt = 你想怎麼稱呼{ $item_class }？
item-name-set = 你給{ $item }命名為「{ $name }」。
item-called-set = 你把{ $item_class }稱為「{ $name }」。
nothing-to-drop = 你沒有東西可以丟棄。
nothing-to-eat = 你沒有東西可以吃。
nothing-to-drink = 你沒有東西可以喝。
nothing-to-read = 你沒有東西可以閱讀。
nothing-to-wield = 你沒有東西可以揮舞。
nothing-to-wear = 你沒有東西可以穿戴。
nothing-to-remove = 你沒有穿戴任何東西可以脫下。
nothing-to-zap = 你沒有東西可以使用。
nothing-to-throw = 你沒有東西可以投擲。
nothing-to-apply = 你沒有東西可以使用。

## ============================================================================
## 戰鬥 — 遠程和投擲
## ============================================================================

throw-weapon = 你投擲了{ $weapon }！
throw-hits-wall = { $projectile }撞到了牆上。
throw-falls-short = { $projectile }沒有飛到目標處。
throw-lands = { $projectile }落在了地上。
throw-breaks = { $projectile }碎裂了！
throw-multishot = 你射出了{ $count }發{ $projectile }！
throw-boomerang-return = { $projectile }飛回了你的手中！
throw-boomerang-miss = { $projectile }沒有飛回來！
ranged-ammo-break = { $projectile }碎了！
ranged-ammo-lost = { $projectile }不見了。
ranged-quiver-empty = 你的箭袋空了。
ranged-no-ammo = 你沒有合適的彈藥。
ranged-not-ready = 你還沒準備好發射。
shoot-hit = { $projectile }擊中了{ $defender }！
shoot-miss = { $projectile }從{ $defender }身旁飛過。
shoot-kill = { $projectile }摧毀了{ $defender }！
multishot-fire = 你向{ $defender }射出了{ $count }發{ $projectile }！
launcher-wield = 你舉起了{ $launcher }。
launcher-no-ammo = 你沒有{ $launcher }的彈藥。

## ============================================================================
## 物品 — 戒指和護身符
## ============================================================================

ring-put-on = 你戴上了{ $ring }。
ring-remove = 你摘下了{ $ring }。
ring-cursed-remove = { $ring }被詛咒了！你無法摘下它。
ring-effect-gain = 你感到{ $effect }。
ring-effect-lose = 你不再感到{ $effect }。
ring-shock = 戒指電擊了你！
ring-hunger = 你感到一陣飢餓。
ring-sink-vanish = { $ring }從你的手指上滑落，消失在排水口中！
amulet-put-on = 你戴上了{ $amulet }。
amulet-remove = 你摘下了{ $amulet }。
amulet-cursed = { $amulet }黏在了你的脖子上！
amulet-strangulation = 護身符勒住了你的脖子！
amulet-lifesave = 你的護身符碎成了碎片！
amulet-lifesave-msg = 但是等等……你的護身符變得溫暖了！
amulet-reflection = { $attack }被你的護身符反射了！

## ============================================================================
## 物品 — 工具
## ============================================================================

tool-apply = 你使用了{ $tool }。
tool-lamp-on = { $lamp }開始發光了。
tool-lamp-off = { $lamp }熄滅了。
tool-lamp-fuel = { $lamp }的燃料用完了。
tool-pick-locked = 你成功撬開了鎖。
tool-pick-fail = 你沒能撬開鎖。
tool-horn-blow = 你吹出了一聲{ $effect }！
tool-mirror-reflect = 你用鏡子反射了{ $attack }！
tool-mirror-look = 你在鏡子裡看到了自己。
tool-stethoscope = 你聽診了{ $target }。
tool-tinning-kit = 你開始將{ $corpse }做成罐頭。
tool-leash-attach = 你把{ $leash }繫在了{ $pet }身上。
tool-leash-detach = 你解開了{ $pet }身上的{ $leash }。
tool-camera-flash = 你對{ $target }拍了張照！
tool-whistle-blow = 你吹了哨子。
tool-whistle-magic = 你吹出了一種奇異的哨音！

## ============================================================================
## 地牢設施 — 特殊房間和事件
## ============================================================================

shop-enter = 你進入了{ $shopkeeper }的{ $shoptype }。
shop-leave = { $shopkeeper }說「歡迎再來！」
shop-price = 「給你，{ $item }只要{ $price }枚金幣。」
shop-price-bargain = 「給你，{ $item }只要{ $price }枚金幣，真是便宜。」
shop-price-excellent-choice = 「給你，{ $item }只要{ $price }枚金幣，絕對是上佳之選。」
shop-price-finest-quality = 「給你，{ $item }只要{ $price }枚金幣，品質上乘。」
shop-price-gourmets-delight = 「給你，{ $item }只要{ $price }枚金幣，美食家的最愛！」
shop-price-painstakingly-developed = 「給你，{ $item }只要{ $price }枚金幣，精心打造！」
shop-price-superb-craftsmanship = 「給你，{ $item }只要{ $price }枚金幣，做工精妙！」
shop-price-one-of-a-kind = 「給你，{ $item }只要{ $price }枚金幣，獨一無二！」
shop-stolen = 你有未付款的商品！
shop-enter-digging-tool = 店裡傳來警告，要你把挖掘工具留在外面。
shop-enter-steed = 店裡傳來聲音，堅持要你把{ $steed }留在外面。
shop-enter-invisible = 店裡傳來懷疑的聲音：隱形顧客不受歡迎。
shop-leave-warning = { $shopkeeper }喊道：「請先付款再離開！」
shop-damage = { $shopkeeper }說「你得賠償損失！」
shop-repair = { $shopkeeper }開始修理店裡的損壞。
shop-keeper-dead = { $shopkeeper }死了，這家店已經廢棄。
shop-shoplift = { $shopkeeper }尖叫道：「站住，小偷！」
temple-enter = 你進入了{ $god }的神殿。
temple-forbidding = 你感到一股令人敬畏又排斥的神聖氣息。
temple-peace = 一股深沉的平靜籠罩著這座神殿。
temple-unusual-peace = 這座神殿顯得異常平靜。
temple-donate = { $priest }接受了你的捐贈。
temple-protection = { $priest }賜予你神聖的保護。
vault-guard = 突然，一名金庫守衛出現了！
vault-guard-ask = 「你是誰？你在這裡做什麼？」

## ============================================================================
## 陷阱 — 擴展陷阱訊息
## ============================================================================

trap-bear-leg = 捕獸夾夾住了你的腿！
trap-bear-escape = 你從捕獸夾中掙脫了。
trap-bear-stuck = 你被捕獸夾困住了！
trap-pit-climb = 你爬出了坑。
trap-pit-cant-climb = 你試圖爬出坑，但失敗了！
trap-spiked-damage = 那些尖刺是有毒的！
trap-arrow-dodge = 你躲開了那支箭！
trap-dart-poison = 那支飛鏢是有毒的！
trap-land-mine = 轟！！你觸發了一枚地雷！
trap-land-mine-set = 你安置了地雷。
trap-sleeping-gas = 一團毒氣讓你昏睡了！
trap-hole = 你從地板上的洞掉了下去！
trap-trapdoor = 你腳下的活板門突然打開了！
trap-magic-trap = 你被魔法爆炸吞沒了！
trap-anti-magic = 你感到魔力被抽空了！
trap-statue = 雕像活了過來！
trap-vibrating-square = 你感到腳下有一種奇異的振動。
trap-seen = 你看到這裡有{ $trap }。
trap-monster-trigger = { $monster }觸發了{ $trap }！
trap-monster-pit = { $monster }掉進了坑裡！
trap-monster-bear = { $monster }被捕獸夾夾住了！
trap-monster-web = { $monster }被蛛網纏住了！
trap-monster-teleport = { $monster }消失了！
trap-set-fail = 你沒能安置好陷阱。
trap-set-success = 你安置了{ $trap }。

## ============================================================================
## 神器 — 特殊效果和訊息
## ============================================================================

artifact-resist = 神器進行了抵抗！
artifact-evade = { $artifact }躲開了你的觸碰！
artifact-blast = { $artifact }灼傷了你！
artifact-glow-fire = { $artifact }散發著神聖的火焰！
artifact-glow-cold = { $artifact }散發著冰藍色的光芒！
artifact-glow-warning = { $artifact }發出警示的光芒！
artifact-invoke = 你喚起了{ $artifact }的力量。
artifact-invoke-fail = 似乎什麼也沒有發生。
artifact-gift = { $god }賜予你{ $artifact }！
artifact-touch-blast = { $artifact }灼燒了你的肌膚！
artifact-speak = { $artifact }對你說話了！
artifact-sing = { $artifact }在你手中吟唱著。
artifact-thirst = { $artifact }渴望鮮血！
artifact-kill-msg = { $artifact }以致命的力量擊中了{ $defender }！
artifact-bisect = { $artifact }將{ $defender }劈成了兩半！
artifact-drain-life = { $artifact }吸取了{ $defender }的生命力！
artifact-found = 你感覺到附近有{ $artifact }的存在。
artifact-already-exists = 這個名字的神器已經存在於這局遊戲中了。
artifact-wish-denied = 你感到手中出現了什麼東西，但隨即消失了！
artifact-name-change = { $artifact }的名字在你眼前變化了！

## ============================================================================
## 商店 — 擴展訊息
## ============================================================================

shop-owe = 你欠{ $shopkeeper }{ $amount }枚金幣。
shop-bill-total = 你的帳單總計{ $amount }枚金幣。
shop-pay-success = 你向{ $shopkeeper }支付了{ $amount }枚金幣。
shop-usage-fee = { $shopkeeper }說道：「使用費，{ $amount }枚金幣。」
shop-no-money = 你沒有足夠的錢！
shop-buy = 你花了{ $price }枚金幣買了{ $item }。
shop-sell = 你以{ $price }枚金幣的價格賣出了{ $item }。
shop-credit = 你有{ $amount }枚金幣的信用額度。
shop-door-block = { $shopkeeper }擋住了門口！
shop-angry = { $shopkeeper }發怒了！
shop-kops = 基石警察來了！
shop-kops-arrive = 基石警察趕到了！
shop-use-unpaid = { $shopkeeper }喊道：「你在使用未付款的商品！」
shop-broke-item = 你弄壞了{ $item }！{ $shopkeeper }要求你賠償{ $price }枚金幣。
shop-welcome-back = { $shopkeeper }說「歡迎回來！你欠了{ $amount }枚金幣。」
shop-closed = 商店關門了。

## ============================================================================
## 宗教 — 祈禱、祭祀、加冕
## ============================================================================

pray-start = 你開始向{ $god }祈禱。
pray-feel-warm = 你感到一陣溫暖的光輝。
pray-feel-at-peace = 你感到內心平靜。
pray-full-heal = 你感覺好多了！
pray-uncurse = 你感覺{ $god }正在幫助你。
pray-resist = 你感到獲得了抗性！
pray-angry-god = { $god }很不高興！
pray-ignored = { $god }似乎沒有在聽。
pray-punish = { $god }懲罰了你！
pray-gift-weapon = { $god }賜予了你一件禮物！
pray-mollified = { $god }似乎不那麼生氣了。
pray-reconciled = { $god }似乎已經原諒了你。
sacrifice-accept = 你的祭品在火焰中被吞噬了！
sacrifice-reject = { $god }不為所動。
sacrifice-already-full = 你有一種不夠格的感覺。
sacrifice-wrong-altar = 你感到愧疚。
sacrifice-convert = 祭壇轉化為{ $god }的了！
sacrifice-gift = { $god }很高興，賜予你一件禮物！
crown-msg = 你聽到一個聲音在迴盪：「汝乃天選之人！」
crown-gain = 你感到{ $god }的力量在體內湧動！

## ============================================================================
## 寵物 — 擴展訊息
## ============================================================================

pet-hungry = 你的{ $pet }看起來很餓。
pet-very-hungry = 你的{ $pet }非常餓！
pet-starving = 你的{ $pet }快餓死了！
pet-refuses-food = 你的{ $pet }拒絕吃{ $food }。
pet-loyal = 你的{ $pet }崇拜地看著你。
pet-growl = 你的{ $pet }向你低吼！
pet-confused = 你的{ $pet }看起來很困惑。
pet-injured = 你的{ $pet }看起來受傷了。
pet-healed = 你的{ $pet }看起來更健康了。
pet-level-up = 你的{ $pet }似乎更有經驗了！
pet-died = 你的{ $pet }被殺死了！
pet-revived = 你的{ $pet }被復活了！
pet-attack-monster = 你的{ $pet }攻擊了{ $monster }！
pet-fetch = 你的{ $pet }撿回了{ $item }。
pet-saddle = 你給你的{ $pet }裝上了鞍。

## ============================================================================
## 飢餓 — 進食效果、屍體效果、固有屬性
## ============================================================================

eat-gain-strength = 你感到力大無窮！
eat-gain-telepathy = 你感到一種奇異的心靈感應。
eat-gain-invisibility = 你感到自己變得透明了！
eat-gain-poison-resist = 你感到身體健康！
eat-gain-fire-resist = 你感到一陣涼意。
eat-gain-cold-resist = 你感到溫暖。
eat-gain-sleep-resist = 你感到精神十足！
eat-gain-shock-resist = 你感到自己被絕緣了。
eat-tainted = 呃——那食物變質了！
eat-corpse-taste = 這具{ $corpse }的味道{ $taste ->
    [terrible] 糟糕透了
    [bland] 很淡
    [okay] 還行
   *[normal] 就像{ $corpse }的味道
}！
eat-petrify = 你感到自己正在變成石頭！
eat-polymorph = 你感到身體正在發生變化！
eat-stun = 你晃了一下。
eat-hallucinate = 哇喔！你感到飄飄欲仙！
eat-acidic = 酸性食物灼燒了你的胃！

## ============================================================================
## 行為準則 — 違反和成就
## ============================================================================

conduct-vegetarian-break = 你打破了素食主義準則。
conduct-vegan-break = 你打破了純素準則。
conduct-foodless-break = 你打破了不進食準則。
conduct-atheist-break = 你打破了無神論準則。
conduct-weaponless-break = 你打破了徒手準則。
conduct-pacifist-break = 你打破了和平主義準則。
conduct-illiterate-break = 你打破了文盲準則。
conduct-genocideless-break = 你打破了不滅絕準則。
conduct-polypileless-break = 你打破了不使用變化堆準則。
conduct-polyself-break = 你打破了不自我變化準則。
achievement-unlock = 成就解鎖：{ $name }！
achievement-sokoban = 你解開了倉庫番的謎題！
achievement-mines-end = 你到達了侏儒礦坑的底部！
achievement-medusa = 你擊敗了美杜莎！
achievement-castle = 你攻破了城堡！
achievement-amulet = 你獲得了Yendor的護身符！

## ============================================================================
## 怪物AI — 物品使用、貪婪行為
## ============================================================================

monster-reads = { $monster }閱讀了一張卷軸！
monster-uses-wand = { $monster }使用了一根{ $wand_type }魔杖！
monster-quaffs = { $monster }喝了一瓶藥水！
monster-puts-on = { $monster }穿上了{ $item }。
monster-removes = { $monster }脫下了{ $item }。
monster-heals = { $monster }看起來更健康了！
monster-teleport-away = { $monster }傳送走了！
monster-covetous-approach = { $monster }氣勢洶洶地逼近了！
monster-covetous-steal = { $monster }從你手中搶走了{ $item }！
monster-covetous-flee = { $monster }帶著{ $item }撤退了！
monster-unlock = { $monster }打開了門鎖。
monster-open-door = { $monster }推開了門。
monster-close-door = { $monster }關上了門。
monster-break-door = { $monster }破開了門！
monster-dig = { $monster }在牆上挖了個洞！

## ============================================================================
## 特殊層級 — 倉庫番、礦坑、神諭者等
## ============================================================================

level-sokoban-enter = 你進入了一個看起來像謎題的房間。
level-sokoban-solve = 喀嗒！你聽到一扇門被打開了。
level-sokoban-cheat = 你聽到一陣隆隆聲。
level-mines-enter = 你進入了侏儒礦坑。
level-mines-town = 你進入了礦鎮。
level-oracle-enter = 你看到一個大房間，中間有一座奇特的噴泉。
level-oracle-speak = 神諭者開口了……
level-oracle-consult = 神諭者願意以{ $price }枚金幣的價格分享智慧。
level-oracle-rumor = 神諭者揭示道：「{ $rumor }」
level-castle-enter = 你進入時感到一陣恐懼。
level-vlad-tower = 你感到一股冰冷的氣息。
level-sanctum-enter = 你有一種奇異的不祥之感……
level-astral-enter = 你到達了星界！

## ============================================================================
## 得分和結局 — 擴展訊息
## ============================================================================

score-display = 分數：{ $score }
score-rank = 你排名第{ $rank }。
score-high-new = 新的最高分！
score-high-list-title = 最高分排行榜
score-high-header = 排名  分數  條目
score-high-row = { $rank }. { $score }分  { $name }，{ $role }（{ $gender } { $race } { $alignment }），{ $cause }，位於 { $depth }
score-high-entry = { $rank }. { $role }{ $name }（{ $score }分）
score-gold-collected = 收集金幣：{ $amount }
score-monsters-killed = 擊殺怪物：{ $count }
score-deepest-level = 到達最深層級：{ $depth }
score-death-by = 在地牢第{ $depth }層被{ $killer }殺死。
score-escaped-with = 你以{ $score }分逃出了地牢。
score-ascended-with = 你以{ $score }分飛升了！
game-over-conduct-title = 自願挑戰：
game-over-conduct-item = 你遵守了{ $conduct }準則。
game-over-dungeon-overview = 地牢概覽：
game-over-vanquished = 被消滅的生物：
game-over-genocided = 被滅絕的物種：


## ============================================================================
## 引擎國際化鍵 — 移動
## ============================================================================

diagonal-squeeze-blocked = 你沒法從那個對角間隙擠過去。
door-no-closed = 那裡沒有關著的門。
door-no-open = 那裡沒有開著的門。
door-no-kick = 那裡沒有門可以踢。
pet-swap = 你和你的寵物交換了位置。
pet-nearby = 你的{ $pet }在附近。

## ============================================================================
## 引擎國際化鍵 — 卷軸（擴展）
## ============================================================================

scroll-identify-one = 你鑑定了一件物品。
scroll-identify-count = 你鑑定了{ $count }件物品。
scroll-nothing-to-identify = 你沒有東西需要鑑定。
scroll-enchant-weapon-fragile = 你的武器感覺變脆弱了。
scroll-enchant-weapon-film = 你的武器覆蓋了一層薄膜。
scroll-enchant-weapon-evaporate = 你的武器蒸發了！
scroll-enchant-weapon-vibrate = 你的武器突然劇烈震動！
scroll-enchant-armor-skin = 你的皮膚閃了一下光然後褪去了。
scroll-enchant-armor-fragile = 你的護甲感覺變脆弱了。
scroll-enchant-armor-film = 你的護甲覆蓋了一層薄膜。
scroll-enchant-armor-evaporate = 你的護甲蒸發了！
scroll-enchant-armor-vibrate = 你的護甲突然劇烈震動！
scroll-remove-curse-malignant = 你感到一種邪惡的氣息包圍著你。
scroll-remove-curse-blessed = 你感到與萬物合一。
scroll-remove-curse-punishment = 你的懲罰被解除了！
scroll-disintegrate = 卷軸碎裂了。
scroll-confuse-cursed = 你的雙手抽搐了一下。
scroll-teleport-disoriented = 你感到非常迷失。
scroll-trap-detection = 你感覺到陷阱的存在。
scroll-scare-wailing = 你聽到遠處傳來悲傷的哀嚎。
scroll-scare-dust = 你撿起卷軸時它化為了灰燼。
scroll-fire-burn = 卷軸著火了，燒傷了你的手。
scroll-earth-rocks = 石頭從你周圍落下！
scroll-earth-boulders = 巨石從你周圍落下！
scroll-amnesia-spells = 你忘記了你的法術！
scroll-destroy-armor-itch = 你的皮膚發癢。
scroll-destroy-armor-glow = 你的護甲發出光芒。
scroll-destroy-armor-crumble = 你的護甲碎裂成灰了！
scroll-taming-growl = 你聽到憤怒的咆哮！
scroll-genocide-guilty = 你感到愧疚。
scroll-genocide-prompt = 你想滅絕什麼怪物？
scroll-genocide-prompt-class = 你想滅絕哪一類怪物？
scroll-light-sparkle = 微光在你周圍閃爍。
scroll-charging-drained = 你感到精力被耗盡。
scroll-charging-id = 這是一張充能卷軸。
scroll-charging-nothing = 你沒有可以充能的東西。
scroll-magic-mapping-fail = 不幸的是，你無法理解那些細節。
scroll-create-monster-horde = 一群怪物出現了！

## ============================================================================
## 引擎國際化鍵 — 陷阱（擴展）
## ============================================================================

trap-arrow-shoot = 一支箭向你射來！
trap-dart-shoot = 一支小飛鏢向你射來！
trap-dart-poison-resist = 飛鏢有毒，但毒藥似乎對你沒有效果。
trap-trapdoor-ceiling = 天花板上的活板門打開了，但什麼也沒有掉下來！
trap-sleeping-gas-sleep = 一團毒氣讓你昏睡了！
trap-fire-resist = 一根火柱從地板噴發了！但你抵抗了效果。
trap-rolling-boulder-trigger = 喀嗒！你觸發了一個滾石陷阱！
trap-teleport-wrench = 你感到一陣扭曲。
trap-web-tear = 你撕裂了蛛網！
trap-web-free = 你從蛛網中掙脫了。
trap-web-stuck = 你被蛛網困住了。
trap-magic-trap-blind = 你被一道閃光弄瞎了！
trap-door-booby = 門上有詭雷！
trap-gas-puff = 一股毒氣吞沒了你！
trap-gas-cloud = 一團毒氣包圍了你！
trap-shock = 你被電擊了！
trap-chest-explode = 轟！！箱子爆炸了！
trap-pit-float = 你從坑裡飄了出來。
trap-bear-rip-free = 你用力掙脫了捕獸夾！
trap-cannot-disarm = 這裡沒有可以拆除的陷阱。
trap-disarm-fail = 你沒能拆除陷阱。

## ============================================================================
## 引擎國際化鍵 — 傳送
## ============================================================================

teleport-random = 你被傳送了！
teleport-controlled = 你想傳送到哪裡？
teleport-invalid-target = 你無法傳送到那裡！
teleport-level = 你被傳送到了另一層！
teleport-same-level = 你顫抖了一下。
teleport-restricted = 一股神秘的力量阻止了你傳送！
teleport-branch = 你感到自己被拉到了地牢的另一個分支！
teleport-monster = 一隻怪物從視野中消失了！
teleport-no-portal = 你感到一陣扭曲，但什麼都沒發生。
teleport-trap-controlled = 你被陷阱傳送了！你有傳送控制能力。
teleport-trap-restricted = 一股神秘的力量阻止了你傳送！

## ============================================================================
## 引擎國際化鍵 — 移動（擴展）
## ============================================================================

ice-slide = 你在冰面上滑行！
ice-fumble-fall = 你在冰面上滑倒了！
water-float-over = 你漂浮在水面上。
water-swim = 你在水中游泳。
water-drown-danger = 你快要淹死了！
lava-float-over = 你漂浮在岩漿上方。
lava-resist = 岩漿灼燒著你，但你抵抗了大部分傷害。
lava-burn = 岩漿嚴重灼傷了你！
fumble-trip = 你被自己的腳絆倒了！

## ============================================================================
## 引擎國際化鍵 — 吞噬
## ============================================================================

engulf-attack-interior = 你攻擊了怪物的內部！
engulf-escaped = 你從吞噬你的怪物中逃脫了！
engulf-monster-dies = 吞噬你的怪物死了！

## ============================================================================
## 引擎國際化鍵 — 藥水（擴展）
## ============================================================================

potion-blindness-cure = 你的視力恢復了。
potion-gain-ability-str = 你感到強壯！
potion-paralysis-brief = 你短暫地僵硬了一下。
potion-no-effect = 你感到缺少了什麼。
potion-sickness-deadly = 你感到病入膏肓。
potion-booze-passout = 你暈過去了。
potion-enlightenment = 你感到自我認知增強了……

## ============================================================================
## 引擎國際化鍵 — 飢餓（擴展）
## ============================================================================

eat-choke = 你被食物噎住了！
eat-dread = 你感到一陣恐懼。
eat-corpse-effect = 你感到吃那具屍體有一種不尋常的效果。
eat-weakened = 你感到虛弱了。
eat-greasy = 你的手指非常油膩。
eat-poison-resist = 你似乎沒有受到毒素的影響。

## ============================================================================
## 引擎國際化鍵 — 宗教（擴展）
## ============================================================================

sacrifice-own-kind-anger = 你因為獻祭同族而激怒了你的神！
sacrifice-own-kind-pleased = 你的神對你獻祭同族感到滿意。
sacrifice-pet-guilt = 你對獻祭你的前寵物感到愧疚。
sacrifice-reduce-timeout = 你的祭品縮短了下次祈禱的等待時間。
pray-partial = 你的祈禱只被部分聽到了。

## ============================================================================
## 引擎國際化鍵 — 神器（擴展）
## ============================================================================

artifact-invoke-heal = 你感覺好了一些。
artifact-invoke-energy = 你感到一股魔力湧入！
artifact-invoke-enlighten = 你感到自我認知增強了……
artifact-invoke-conflict = 你感到自己像一個煽動者。
artifact-invoke-invisible = 你感到自己變得相當透明了。
artifact-invoke-levitate = 你開始漂浮在空中！
artifact-invoke-untrap = 你感到擅長拆除陷阱。
artifact-invoke-charge = 你可以為一件物品充能。
artifact-invoke-teleport = 你感到一陣扭曲。
artifact-invoke-portal = 你感到空氣中有一種微光。
artifact-invoke-arrows = 一陣箭雨出現了！
artifact-invoke-brandish = 你威風凜凜地揮舞著神器！
artifact-invoke-venom = 你甩出了一團毒液！
artifact-invoke-cold = 一陣寒氣爆發了！
artifact-invoke-fire = 一個火球爆發了！
artifact-invoke-light = 一道致盲的光線射出！

## ============================================================================
## 引擎國際化鍵 — 魔杖（擴展）
## ============================================================================

wand-enlightenment = 你感到自我認知增強了。
wand-secret-door-detect = 你感覺到暗門的存在。

## ============================================================================
## 引擎國際化鍵 — 商店（擴展）
## ============================================================================

shop-free = 你免費得到了那個！
shop-return = { $shopkeeper }接受了退貨。
shop-not-interested = { $shopkeeper }不感興趣。
shop-angry-take = 「謝謝你，賤人！」
shop-restock = { $shopkeeper }似乎對補貨很感激。
shop-no-debt = 你不欠任何東西。
shop-credit-covers = 你的信用額度支付了帳單。
shop-stolen-amount = 你偷走了價值{ $amount }枚金幣的商品。

## ============================================================================
## 物品命名 — BUC 狀態標籤
## ============================================================================

item-buc-blessed = 祝福的
item-buc-uncursed = 未詛咒的
item-buc-cursed = 被詛咒的

## ============================================================================
## 物品命名 — 侵蝕形容詞
## ============================================================================

item-erosion-rusty = 生鏽的
item-erosion-very-rusty = 非常鏽的
item-erosion-thoroughly-rusty = 鏽透的
item-erosion-corroded = 腐蝕的
item-erosion-very-corroded = 非常腐蝕的
item-erosion-thoroughly-corroded = 徹底腐蝕的
item-erosion-burnt = 燒焦的
item-erosion-very-burnt = 嚴重燒焦的
item-erosion-thoroughly-burnt = 徹底燒焦的
item-erosion-rotted = 腐爛的
item-erosion-very-rotted = 非常腐爛的
item-erosion-thoroughly-rotted = 徹底腐爛的
item-erosion-rustproof = 防鏽的
item-erosion-fireproof = 防火的
item-erosion-corrodeproof = 防腐蝕的
item-erosion-rotproof = 防腐爛的

## ============================================================================
## 物品命名 — 類別特定基礎名稱模式
## ============================================================================

item-potion-identified = { $name }藥水
item-potion-called = 被稱為{ $called }的藥水
item-potion-appearance = { $appearance }藥水
item-potion-generic = 藥水

item-scroll-identified = { $name }捲軸
item-scroll-called = 被稱為{ $called }的捲軸
item-scroll-labeled = 標記為{ $label }的捲軸
item-scroll-appearance = { $appearance }捲軸
item-scroll-generic = 捲軸

item-wand-identified = { $name }魔杖
item-wand-called = 被稱為{ $called }的魔杖
item-wand-appearance = { $appearance }魔杖
item-wand-generic = 魔杖

item-ring-identified = { $name }戒指
item-ring-called = 被稱為{ $called }的戒指
item-ring-appearance = { $appearance }戒指
item-ring-generic = 戒指

item-amulet-called = 被稱為{ $called }的護身符
item-amulet-appearance = { $appearance }護身符
item-amulet-generic = 護身符

item-spellbook-identified = { $name }魔法書
item-spellbook-called = 被稱為{ $called }的魔法書
item-spellbook-appearance = { $appearance }魔法書
item-spellbook-generic = 魔法書

item-gem-stone = 石頭
item-gem-gem = 寶石
item-gem-called-stone = 被稱為{ $called }的石頭
item-gem-called-gem = 被稱為{ $called }的寶石
item-gem-appearance-stone = { $appearance }石頭
item-gem-appearance-gem = { $appearance }寶石

item-generic-called = 被稱為{ $called }的{ $base }

## ============================================================================
## 物品命名 — 連接詞和後綴
## ============================================================================

item-named-suffix = 「{ $name }」

## ============================================================================
## 物品命名 — 冠詞
## ============================================================================

item-article-the = 那個
item-article-your = 你的

## ============================================================================
## 物品命名 — 複數選擇
## ============================================================================

item-count-name = { $count ->
   *[other] { $singular }
}

## ============================================================================
## 狀態欄標籤
## ============================================================================

status-satiated = 飽食
status-hungry = 飢餓
status-weak = 虛弱
status-fainting = 昏厥
status-not-hungry = {""}
status-starved = 餓死

## ============================================================================
## 界面 — 標題和標籤
## ============================================================================

ui-inventory-title = 物品欄
ui-inventory-empty = 你沒有攜帶任何東西。
ui-equipment-title = 裝備
ui-equipment-empty = 你沒有穿戴任何特殊裝備。
ui-help-title = NetHack Babel 幫助
ui-message-history-title = 訊息歷史
ui-select-language = 選擇語言
ui-more = --更多--
ui-save-prompt = 正在儲存遊戲...
ui-save-success = 遊戲已儲存。
ui-save-goodbye = 遊戲已儲存。再見！
ui-goodbye = 再見！
ui-game-over-thanks = 遊戲結束。感謝遊玩！
ui-unknown-command = 未知命令：'{ $key }'。按 ? 查看幫助。

## ============================================================================
## 界面 — 提示
## ============================================================================

prompt-drop = 丟棄什麼？[a-zA-Z 或 ?*]
prompt-wield = 裝備什麼武器？[a-zA-Z 或 - 徒手]
prompt-wear = 穿戴什麼？[a-zA-Z 或 ?*]
prompt-takeoff = 脫下什麼？[a-zA-Z 或 ?*]
prompt-puton = 戴上什麼？[a-zA-Z 或 ?*]
prompt-remove = 摘下什麼？[a-zA-Z 或 ?*]
prompt-apply = 使用什麼？[a-zA-Z 或 ?*]
prompt-throw-item = 投擲什麼？[a-zA-Z 或 ?*]
prompt-throw-dir = 朝哪個方向？
prompt-zap-item = 揮動什麼？[a-zA-Z 或 ?*]
prompt-zap-dir = 朝哪個方向？
prompt-open-dir = 朝哪個方向打開？
prompt-close-dir = 朝哪個方向關閉？
prompt-fight-dir = 朝哪個方向攻擊？
prompt-pickup = 撿起什麼？
prompt-dip-item = 蘸什麼？[a-zA-Z]
prompt-dip-into = 蘸入什麼？[a-zA-Z]

## ============================================================================
## 界面 — 物品欄分類標題
## ============================================================================

inv-class-weapon = 武器
inv-class-armor = 防具
inv-class-ring = 戒指
inv-class-amulet = 護身符
inv-class-tool = 工具
inv-class-food = 食物
inv-class-potion = 藥水
inv-class-scroll = 捲軸
inv-class-spellbook = 魔法書
inv-class-wand = 魔杖
inv-class-coin = 金幣
inv-class-gem = 寶石
inv-class-rock = 石頭
inv-class-ball = 鐵球
inv-class-chain = 鐵鏈
inv-class-venom = 毒液
inv-class-other = 其他

# 物品欄 BUC 標記
inv-buc-marker-blessed = [祝]
inv-buc-marker-cursed = [咒]
inv-buc-tag-blessed = （祝福）
inv-buc-tag-cursed = （詛咒）
inv-buc-tag-uncursed = （未詛咒）
ui-pickup-title = 拾取什麼？

## ============================================================================
## 事件訊息
## ============================================================================

event-hp-gained = 你感覺好多了。
event-hp-lost = 哎喲！
event-pw-gained = 你感到魔力回湧。
event-you-see-here = 你看到這裡有{ $terrain }。
event-dungeon-welcome = 你發現自己身處一座地牢中。祝你好運！
event-player-role = 你是{ $align }{ $race }{ $role }{ $name }。

## ============================================================================
## 地形名稱
## ============================================================================

terrain-floor = 地板
terrain-corridor = 走廊
terrain-stone = 實心岩壁
terrain-wall = 牆壁
terrain-closed-door = 關閉的門
terrain-open-door = 打開的門
terrain-locked-door = 鎖住的門
terrain-stairs-up = 向上的樓梯
terrain-stairs-down = 向下的樓梯
terrain-fountain = 噴泉
terrain-altar = 祭壇
terrain-throne = 王座
terrain-sink = 水槽
terrain-grave = 墳墓
terrain-pool = 水池
terrain-moat = 護城河
terrain-ice-terrain = 冰面
terrain-air = 空氣
terrain-cloud = 雲層
terrain-water = 水
terrain-lava = 岩漿
terrain-trap = 陷阱
terrain-tree = 樹
terrain-iron-bars = 鐵柵欄
terrain-drawbridge = 吊橋
terrain-magic-portal = 魔法傳送門

## ============================================================================
## 引擎 — 陷阱訊息
## ============================================================================

trap-shiver = 你突然打了個寒顫。
trap-howl = 你聽到遠處的嚎叫聲。
trap-yearning = 你感到一陣奇怪的渴望。
trap-pack-shakes = 你的背包劇烈搖晃！
trap-fumes = 你聞到刺鼻的煙霧。
trap-tired = 你突然感到很累。

## ============================================================================
## 引擎 — 鑑定類別名稱
## ============================================================================

id-class-potion = 藥水
id-class-scroll = 捲軸
id-class-ring = 戒指
id-class-wand = 魔杖
id-class-spellbook = 魔法書
id-class-amulet = 護身符
id-class-weapon = 武器
id-class-armor = 防具
id-class-tool = 工具
id-class-food = 食物
id-class-coin = 金幣
id-class-gem = 寶石
id-class-rock = 石頭
id-class-ball = 鐵球
id-class-chain = 鐵鏈
id-class-venom = 毒液
id-class-unknown = 東西
id-unknown-object = 奇怪的物體
id-something = 某物

## ============================================================================
## 引擎 — 商店類型名稱
## ============================================================================

shop-type-general = 雜貨店
shop-type-armor = 二手鎧甲店
shop-type-book = 二手書店
shop-type-liquor = 酒莊
shop-type-weapon = 古董武器店
shop-type-deli = 熟食店
shop-type-jewel = 珠寶店
shop-type-apparel = 高級服飾店
shop-type-hardware = 五金店
shop-type-rare-book = 珍本書店
shop-type-health = 保健食品店
shop-type-lighting = 燈具店

## ============================================================================
## 引擎 — 寵物種類名稱
## ============================================================================

pet-little-dog = 小狗
pet-kitten = 小貓
pet-pony = 小馬

## ============================================================================
## 引擎 — 陣營名稱
## ============================================================================

align-law = 秩序
align-balance = 中立
align-chaos = 混沌

## ============================================================================
## BUC 標記（物品欄顯示）
## ============================================================================

buc-tag-blessed = （祝福）
buc-tag-cursed = （詛咒）
buc-tag-uncursed = （未詛咒）
buc-marker-blessed = [祝]
buc-marker-cursed = [咒]

## ============================================================================
## 角色創建 — 職業
## ============================================================================

role-archeologist = 考古學家
role-barbarian = 野蠻人
role-caveperson = 穴居人
role-healer = 治癒者
role-knight = 騎士
role-monk = 武僧
role-priest = 牧師
role-ranger = 遊俠
role-rogue = 盜賊
role-samurai = 武士
role-tourist = 旅行者
role-valkyrie = 女武神
role-wizard = 巫師

## ============================================================================
## 角色創建 — 種族
## ============================================================================

race-human = 人類
race-elf = 精靈
race-dwarf = 矮人
race-gnome = 侏儒
race-orc = 獸人

## ============================================================================
## 角色創建 — 陣營
## ============================================================================

alignment-lawful = 守序
alignment-neutral = 中立
alignment-chaotic = 混沌

## ============================================================================
## 角色創建 — 提示
## ============================================================================

chargen-pick-role = 選擇職業：
chargen-pick-race = 選擇種族：
chargen-pick-alignment = 選擇陣營：
chargen-who-are-you = 你叫什麼名字？[預設：{ $default }]

## ============================================================================
## 狀態列標籤 — 第一行（屬性）
## ============================================================================

stat-label-str = 力
stat-label-dex = 敏
stat-label-con = 體
stat-label-int = 智
stat-label-wis = 感
stat-label-cha = 魅

## ============================================================================
## 狀態列標籤 — 第二行（地下城狀態）
## ============================================================================

stat-label-dlvl = 深度
stat-label-gold = $
stat-label-hp = 生命
stat-label-pw = 魔力
stat-label-ac = 防禦
stat-label-xp = 經驗
stat-label-turn = 輪
stat-status-blind = 盲
stat-status-conf = 亂
stat-status-stun = 暈
stat-status-hallu = 幻
stat-status-lev = 浮
stat-status-ill = 病
stat-enc-burdened = 負重
stat-enc-stressed = 重負
stat-enc-strained = 吃力
stat-enc-overtaxed = 超載
stat-enc-overloaded = 過載
stat-branch-mines = 礦坑
stat-branch-sokoban = 推箱
stat-branch-quest = 任務
stat-branch-gehennom = 地獄
stat-branch-vlad = 弗拉德
stat-branch-knox = 諾克斯
stat-branch-earth = 地
stat-branch-air = 風
stat-branch-fire = 火
stat-branch-water = 水
stat-branch-astral = 星界
stat-branch-end = 終

## ============================================================================
## 設定選單
## ============================================================================

ui-options-title = 設定
ui-options-game = 遊戲設定
ui-options-display = 顯示設定
ui-options-sound = 音效設定

## 遊戲選項

opt-autopickup = 自動拾取
opt-autopickup-types = 自動拾取類型
opt-legacy = 開場敘事

## 顯示選項

opt-map-colors = 地圖顏色
opt-message-colors = 訊息顏色
opt-buc-highlight = 祝福/詛咒高亮
opt-minimap = 小地圖
opt-mouse-hover = 滑鼠懸停資訊
opt-nerd-fonts = Nerd 字體

## 音效選項

opt-sound-enabled = 音效
opt-volume = 音量

## 選項值

opt-on = 開
opt-off = 關
ui-options-game-title = 遊戲設定（共 { $count } 項）
ui-options-edit-prompt = { $option } [{ $current }]：
ui-oracle-menu-title = 諮詢神諭者
ui-oracle-minor-option = 小型諮詢（50 金幣）
ui-oracle-major-option = 大型諮詢（{ $amount } 金幣）
ui-cancel = 取消
ui-choice-prompt = 選擇>
ui-demon-bribe-prompt = 你要出價多少？[0..{ $amount }]（留空表示拒絕，Esc 取消）
ui-demon-bribe-text-title = 你要出價多少？[0..{ $amount }]
ui-demon-bribe-help = 留空或非法輸入都會視為拒絕
ui-offer-prompt = 出價>
ui-confirm-quit = 真的要退出嗎？
ui-save-failed = 儲存失敗：{ $error }
ui-save-create-dir-failed = 建立存檔目錄失敗：{ $error }
ui-save-load-warning = 警告：讀取存檔失敗：{ $error }
ui-save-load-new-game = 將開始新遊戲。
ui-commands-title = 命令
ui-here-commands-title = 此處相關命令
ui-there-commands-title = 目標位置相關命令
ui-direction-prompt = 往哪個方向？
ui-direction-prompt-optional = 往哪個方向？（Esc 取消）
ui-direction-prompt-run = 朝哪個方向奔跑？
ui-direction-prompt-rush = 朝哪個方向猛衝？
ui-direction-help-title = 方向鍵幫助
ui-direction-help-body = h 向左，j 向下，k 向上，l 向右；y 左上，u 右上，b 左下，n 右下；. 原地，< 上樓，> 下樓；Esc 取消，? 顯示此幫助
ui-direction-invalid = 這不是方向輸入。按 ? 查看幫助。
ui-no-previous-command = 沒有可重複的上一條命令。
ui-press-any-key-continue = （按任意鍵繼續）
ui-count-prefix = 計數
ui-recording-start = 正在錄製會話到：{ $path }
ui-recording-saved = 會話已錄製到：{ $path }
ui-recording-save-warning = 警告：保存錄製失敗：{ $error }
ui-options-volume-prompt = { $option }（0-100）：
ui-text-commands-summary = 指令：h/j/k/l/y/u/b/n 移動，. 原地等待，s 搜尋，, 拾取，i 背包，eq 裝備，p 祈禱，< 上樓，> 下樓，q 離開，? 幫助
ui-text-status-line = 深度:{ $depth }  { $hp }  回合:{ $turn }  位置:{ $pos }  [hjklyubn=移動 .=等待 <=上樓 >=下樓 q=離開 ?=幫助]
ui-startup-loaded-filesystem = 已從 { $path } 載入 { $monsters } 種怪物、{ $objects } 種物品
ui-startup-loaded-embedded = 已從內嵌資源載入 { $monsters } 種怪物、{ $objects } 種物品
ui-startup-language = 目前語言：{ $code }（{ $name }）
ui-restored-save = 已恢復存檔：回合 { $turn }，深度 { $depth }。

option-label-autopickup = 自動拾取
option-label-autodig = 自動挖掘
option-label-autoopen = 自動開門
option-label-autoquiver = 自動準備投射物
option-label-cmdassist = 命令輔助
option-label-confirm = 危險操作確認
option-label-extmenu = 擴展命令選單
option-label-fireassist = 射擊輔助
option-label-fixinv = 固定物品欄字母
option-label-force-invmenu = 強制使用物品選單
option-label-lootabc = 戰利品字母選單
option-label-number-pad = 數字鍵盤移動
option-label-pickup-stolen = 允許拾取贓物
option-label-pickup-thrown = 自動撿回投擲物
option-label-pushweapon = 壓入上一把武器
option-label-quick-farsight = 快速遠望
option-label-rest-on-space = 空白鍵休息
option-label-safe-pet = 保護寵物
option-label-safe-wait = 安全等待
option-label-sortpack = 整理背包
option-label-travel = 自動尋路
option-label-verbose = 詳細訊息
option-label-autopickup-types = 自動拾取類別
option-label-menustyle = 選單樣式
option-label-pile-limit = 物品堆上限
option-label-runmode = 連續移動模式
option-label-sortloot = 戰利品排序
option-label-color = 彩色顯示
option-label-dark-room = 黑暗房間渲染
option-label-hilite-pet = 高亮寵物
option-label-hilite-pile = 高亮物品堆
option-label-lit-corridor = 點亮走廊
option-label-sparkle = 閃爍特效
option-label-standout = 突出顯示
option-label-use-inverse = 反色強調
option-label-hitpointbar = 生命條
option-label-showexp = 顯示經驗
option-label-showrace = 顯示種族
option-label-showscore = 顯示分數
option-label-time = 顯示時間
option-label-fruit = 自訂水果名
option-label-name = 角色名稱
option-label-packorder = 背包順序
option-label-tombstone = 墓碑畫面
option-label-mail = 郵件通知

option-value-traditional = 傳統
option-value-combination = 組合
option-value-full = 完整
option-value-partial = 簡略
option-value-teleport = 傳送
option-value-run = 奔跑
option-value-walk = 行走
option-value-crawl = 爬行
option-value-none = 無
option-value-loot = 僅戰利品

## ============================================================================
## 傳承序言
## ============================================================================

legacy-intro =
    《{ $deity }之書》中寫道：

        創世之後，殘忍的神莫洛克叛變了
        創造者馬杜克的權威。
        莫洛克從馬杜克手中偷走了眾神最強大的
        神器——耶恩德的護身符，
        將其藏匿在黑暗的深淵之中——
        冥府格亨諾姆，他在那裡潛伏至今，
        等待時機。

    你的神{ $deity }渴望擁有護身符，
    並藉此在眾神之上獲得應有的至高地位。

    你，一名初出茅廬的{ $role }，
    從出生起就被預言為{ $deity }的使者。
    你註定要為你的神找回護身符，
    或在嘗試中死去。你命運的時刻已經到來。
    為了我們所有人：勇敢地與{ $deity }同行！

## ============================================================================
## TUI 常用訊息
## ============================================================================

ui-never-mind = 沒關係。
ui-no-such-item = 你沒有那個物品。
ui-not-implemented = 尚未實現。
ui-empty-handed = 你空手著。

## ============================================================================
## 動作分派
## ============================================================================

eat-generic = 你吃了食物。
eat-what = 吃什麼？
quaff-generic = 你喝了藥水。
quaff-what = 喝什麼？
read-generic = 你讀了捲軸。
read-what = 讀什麼？
zap-generic = 你使用了魔杖。

## 門
door-open-success = 門開了。
door-already-open = 這扇門已經開了。
door-not-here = 那裡沒有門。
door-close-success = 門關上了。
door-already-closed = 這扇門已經關了。

## 鎖
lock-nothing-to-force = 這裡沒有可以強行打開的東西。

## 祈禱
pray-begin = 你開始向神靈祈禱……

## 祭品
offer-generic = 你在祭壇上獻上了祭品。
offer-amulet-rejected = 護符被拒絕了，並落在你附近！
offer-what = 獻上什麼？

## 對話
npc-chat-no-response = 這個生物似乎不想聊天。
npc-chat-sleeping = 這個生物似乎根本沒注意到你。
npc-chat-deaf-response = 就算對方回應了，你也聽不見。
chat-nobody-there = 那裡沒有人可以交談。
chat-up = 上面的人聽不見你說話。
chat-down = 下面的人聽不見你說話。
chat-self = 自言自語對地牢冒險者可不是什麼好習慣。
chat-cannot-speak = 以 { $form } 的形態，你無法說話。
chat-strangled = 你說不出話來。你快窒息了！
chat-swallowed = 外面的人聽不見你說話。
chat-underwater = 在水下，你的話誰也聽不清。
chat-statue = 雕像似乎根本沒注意到你。
chat-wall = 這簡直就像在對著牆說話。
chat-wall-hallu-gripes = 牆壁開始抱怨自己的差事。
chat-wall-hallu-joke = 牆壁給你講了個很好笑的笑話！
chat-wall-hallu-insults = 牆壁狠狠辱罵了你的出身！
chat-wall-hallu-uninterested = 牆壁看起來對你毫無興趣。

## 移動/旅行
peaceful-monster-blocks = 你停了下來。{ $monster } 擋住了去路。
ride-not-available = 這裡沒有可以騎乘的東西。
enhance-not-available = 你現在無法提升任何技能。
enhance-success = 你的 { $skill } 提升到了 { $level }。
travel-not-implemented = 旅行功能尚未實現。
two-weapon-not-implemented = 雙武器戰鬥尚未實現。
two-weapon-enabled = 你開始雙武器戰鬥。
two-weapon-disabled = 你停止雙武器戰鬥。
name-not-implemented = 命名功能尚未實現。
adjust-not-implemented = 物品調整功能尚未實現。

## ============================================================================
## 任務/NPC 對話
## ============================================================================

quest-leader-greeting = 歡迎，{ $role }。我一直在等你。
quest-assignment =
    聽好了，{ $role }。{ $nemesis }偷走了{ $artifact }。
    你必須深入地下找回它。
    我們的命運全靠你了。

## 店主
shop-welcome = 歡迎來到{ $shopkeeper }的{ $shoptype }！
shop-buy-prompt = { $shopkeeper }說：「現金還是賒帳？」
shop-unpaid-warning = { $shopkeeper }說：「你有未付款的物品！」
shop-theft-warning = { $shopkeeper }大喊：「小偷！快付錢！」

## 祭司
priest-welcome = 祭司向你唸誦了祝福。
priest-protection-offer = 祭司提供神聖保護，需要{ $cost }金幣。
priest-donation-thanks = 祭司感謝你慷慨的捐贈。
priest-ale-gift = 祭司給了你 { $amount } 枚金幣買酒。
priest-cheapskate = 祭司懷疑地看著你寒酸的捐贈。
priest-small-thanks = 祭司感謝你盡力拿出的這點捐贈。
priest-pious = 祭司說你確實相當虔誠。
priest-clairvoyance = 祭司賜予你片刻的洞察。
status-clairvoyance-end = 你的洞察逐漸消退。
priest-selfless-generosity = 祭司深深感激你無私的慷慨。
priest-cleansing = 祭司的祝福減輕了你的精神負擔。
priest-cranky-1 = 祭司厲聲道：「你還想說話？那我就跟你說道說道！」
priest-cranky-2 = 祭司冷聲道：「想聊天？這就是我要說的話！」
priest-cranky-3 = 祭司說道：「朝聖者，我已不想再同你多言。」

## ============================================================================
## 內容傳遞（謠言、神諭）
## ============================================================================

rumor-fortune-cookie = 你打開了幸運餅乾，上面寫著：「{ $rumor }」
oracle-consultation = { $text }
oracle-no-mood = 神諭者現在沒有心情接受諮詢。
oracle-no-gold = 你身上一枚金幣也沒有。
oracle-not-enough-gold = 你連這個價錢都付不起！

## ============================================================================
## 符號識別
## ============================================================================

whatis-prompt = 你想識別什麼？（選擇一個位置）
whatis-terrain = { $description }（地形）
whatis-monster = { $description }（怪物）
whatis-object = { $description }（物品）
whatis-nothing = 你沒有看到什麼特別的東西。

## 發現
discoveries-title = 已發現
discoveries-empty = 你還沒有發現任何東西。

## ═══ Untranslated (English fallback) ═══

already-mounted = 你已經騎在坐騎上了。

already-punished = 你已經在受罰中了。

attack-acid-hit = 酸液濺了你一身！

attack-acid-resisted = 酸液似乎傷不到你。

attack-breath = { $monster } 朝你噴吐！

attack-cold-hit = 寒氣瞬間吞沒了你！

attack-cold-resisted = 你只是覺得微微發冷。

attack-disease = 你覺得病得厲害。

attack-disintegrate = 你被解離了！

attack-disintegrate-resisted = 你沒有被解離。

attack-drain-level = 你感到生命力正從體內流失！

attack-engulf = { $monster } 把你吞沒了！

attack-fire-hit = 火焰吞沒了你！

attack-fire-resisted = 你只是覺得微微發熱。

attack-hug-crush = 你正被越勒越緊！

attack-paralyze = 你僵在原地，動彈不得！

attack-poisoned = 你覺得中毒了！

attack-shock-hit = 電流狠狠擊中了你！

attack-shock-resisted = 你只是感到一陣輕微麻刺。

attack-sleep = 你昏昏欲睡……

attack-slowed = 你覺得自己動作變慢了。

attack-stoning-start = 你開始變成石頭了！

boulder-blocked = 這塊巨石被卡住了。

boulder-fills-pit = 巨石填平了陷坑！

boulder-push = 你推動了巨石。

call-empty-name = 你沒有取名字。

cannot-do-that = 你不能那麼做。

choke-blood-trouble = 你感到呼吸困難。

choke-consciousness-fading = 你的意識正在消退……

choke-gasping-for-air = 你正拼命喘著氣！

choke-hard-to-breathe = 你感到呼吸困難。

choke-neck-constricted = 你的脖子被勒緊了！

choke-neck-pressure = 你感到脖子上有股壓力。

choke-no-longer-breathe = 你再也無法呼吸！

choke-suffocate = 你窒息而死。

choke-turning-blue = 你的臉都發青了。

chronicle-empty = 你的編年史是空的。

clairvoyance-nothing-new = 你沒有感知到什麼新東西。

container-put-in = 你把 { $item } 放進了 { $container } 裡。

container-take-out = 你從 { $container } 中取出了 { $item }。

crystal-ball-cloudy = 你只看到一團翻騰旋轉的混沌。

crystal-ball-nothing-new = 你沒有看到什麼新東西。

cursed-cannot-remove = 你拿不下來，它被詛咒了！

detect-food-none = 你沒有感知到任何食物。

detect-gold-none = 你沒有感知到任何黃金。

detect-monsters-none = 你沒有感知到任何怪物。

detect-objects-none = 你沒有感知到任何物品。

detect-traps-none = 你沒有感知到任何陷阱。

dig-blocked = 這裡太硬了，挖不動。

dig-floor-blocked = 這裡的地板太硬，挖不動。

dig-floor-hole = 你在地板上挖出了一個洞！

dig-ray-nothing = 挖掘射線沒有作用。

dig-wall-done = 你挖穿了這面牆。

dip-acid-nothing = 什麼也沒有發生。

dip-acid-repair = 你的 { $item } 看起來完好如新！

dip-amethyst-cure = 你感到不那麼困惑了。

dip-diluted = 你的 { $item } 被稀釋了。

dip-excalibur = 當你把劍浸入其中時，一道奇異的光芒掠過劍身！你的劍現在名為 Excalibur！

dip-fountain-cursed = 水面短暫地發出微光。

dip-fountain-nothing = 看起來什麼也沒有發生。

dip-fountain-rust = 你的 { $item } 生鏽了！

dip-holy-water = 你把 { $item } 浸入了聖水中。

dip-no-fountain = 這裡沒有可供浸泡的噴泉。

dip-not-a-potion = 那不是藥水！

dip-nothing-happens = 看起來什麼也沒有發生。

dip-poison-weapon = 你給 { $item } 塗上了毒藥。

dip-unholy-water = 你把 { $item } 浸入了不潔之水中。

dip-unicorn-horn-cure = 你感覺好多了。

djinni-from-bottle = 一個巨大的燈神從瓶中現身！

drawbridge-destroyed = 吊橋被毀掉了！

drawbridge-lowers = 吊橋放下來了！

drawbridge-raises = 吊橋升起來了！

drawbridge-resists = 吊橋沒有反應！

end-ascension-offering = 你向 { $god } 獻上了 Yendor 護符……

end-do-not-pass-go = 不要經過起點，也不要領取 200 佐克幣。

engrave-elbereth = 你在地上刻下了「Elbereth」。

engulf-ejected = 你被 { $monster } 吐了出來！

engulf-escape-killed = 你在 { $monster } 體內將它殺死了！

fire-no-ammo = 你沒有合適的東西可射擊。

fountain-chill = 你感到一陣寒意。

fountain-curse-items = 你頓時有種失落感。

fountain-dip-curse = 水面閃了一下光。

fountain-dip-nothing = 看起來什麼也沒有發生。

fountain-dip-uncurse = 水面閃了一下光。

fountain-dried-up = 噴泉已經乾涸了！

fountain-dries-up = 噴泉乾涸了！

fountain-find-gem = 你感到這裡有顆寶石！

fountain-foul = 水變髒了！你乾嘔並吐了出來。

fountain-gush = 水從滿溢的噴泉裡湧了出來！

fountain-no-position = 你不能從這個位置去浸泡。

fountain-not-here = 這裡沒有噴泉。

fountain-nothing = 一顆大氣泡冒上水面後破掉了。

fountain-poison = 水被污染了！

fountain-refresh = 清涼的氣息讓你精神一振。

fountain-see-invisible = 你感到自己有了自知之明……

fountain-see-monsters = 你感到邪惡的存在。

fountain-self-knowledge = 你感到自己有了自知之明……

fountain-shimmer = 你看見一汪閃爍的水池。

fountain-tingling = 一陣奇異的刺麻感沿著你的手臂竄上來。

fountain-water-demon = 無盡的蛇群從裡面傾瀉而出！

fountain-water-moccasin = 無盡的蛇群從裡面傾瀉而出！

fountain-water-nymph = 一縷薄霧從噴泉中逸散出來……

ghost-from-bottle = 當你打開瓶子時，裡面冒出了什麼東西。

god-lightning-bolt = 突然，一道閃電擊中了你！

grave-corpse = 你在墳墓裡發現了一具屍體。

grave-empty = 這座墳墓裡空無一物。真奇怪……

guard-halt = "站住，小偷！你被捕了！"

guard-no-gold = 衛兵沒在你身上搜到金幣。

guardian-angel-appears = 一位守護天使出現在你身旁！
guardian-angel-rebukes = 你的守護天使斥責了你！

hunger-faint = 你因缺乏食物而昏倒了。

hunger-starvation = 你因飢餓而死。

instrument-no-charges = 這件樂器已經沒有充能了。

intrinsic-acid-res-temp = 你感到一陣短暫的刺痛。

intrinsic-cold-res = 你感覺自己滿肚子熱氣。

intrinsic-disint-res = 你覺得自己非常堅實。

intrinsic-fire-res = 你感到一陣短暫的寒意。

intrinsic-invisibility = 你覺得自己輕飄飄的。

intrinsic-poison-res = 你感覺很健康。

intrinsic-see-invisible = 你覺得自己洞察敏銳！

intrinsic-shock-res = 你感覺自己的生命力被放大了！

intrinsic-sleep-res = 你覺得自己精神抖擻。

intrinsic-stone-res-temp = 你覺得自己格外靈活。

intrinsic-strength = 你覺得自己力大無窮！

intrinsic-telepathy = 你感到精神異常敏銳。

intrinsic-teleport-control = 你感到自己能掌控自身。

intrinsic-teleportitis = 你感到自己很焦躁不安。

invoke-no-power = 看起來什麼也沒有發生。

invoke-not-wielded = 你必須持在手上才能喚起它的力量。

jump-no-ability = 你不知道怎麼跳躍。

jump-out-of-range = 那個地方太遠了！

jump-success = 你跳了起來！

jump-too-burdened = 你負重太大，跳不起來！

kick-door-held = 門被頂住了！

kick-door-open = 你一腳把門踹開了！

kick-hurt-foot = 哎喲！好痛！

kick-item-blocked = 有東西擋住了你的踢擊。

kick-item-moved = 你踢到了什麼東西。

kick-nothing = 你朝空處踢了一腳。

kick-sink-ring = 水槽裡有什麼東西叮噹作響。

known-nothing = 你現在還什麼都不知道。

levitating-cant-go-down = 你漂浮在地板上方很高的地方。

levitating-cant-pickup = 你夠不到地面。

levitation-float-lower = 你輕輕飄回到了地面上。

levitation-wobble = 你在半空中搖搖晃晃。

light-extinguished = 你的 { $item } 熄滅了。

light-lit = 你的 { $item } 現在點亮了。

light-no-fuel = 你的 { $item } 沒有燃料了。

light-not-a-source = 那不是光源。

lizard-cures-confusion = 你感到不那麼困惑了。

lizard-cures-stoning = 你感覺身體靈活多了！

lock-already-locked = 它已經鎖上了。

lock-door-locked = 門鎖上了。

lock-force-container-success = 你強行撬開了鎖！

lock-force-fail = 你沒能強行打開鎖。

lock-force-success = 你強行撬開了鎖！

lock-lockpick-breaks = 你的開鎖器斷掉了！

lock-need-key = 你需要一把鑰匙來鎖上它。

lock-no-door = 這裡沒有門。

lock-no-target = 這裡沒有可供上鎖或開鎖的東西。

lock-pick-container-success = 你成功撬開了鎖。

lock-pick-fail = 你沒能撬開這把鎖。

lock-pick-success = 你成功撬開了鎖。

lycanthropy-cured = 你感到自己被淨化了。

lycanthropy-full-moon-transform = 今晚你覺得渾身發熱。

lycanthropy-infected = 你覺得渾身發熱。

magic-mapping-nothing-new = 你已經清楚周圍的環境了。

mhitm-passive-stoning = { $monster } 變成了石頭！

monster-ability-used = { $monster } 使用了特殊能力！

monster-no-ability = 你目前的形態沒有那種能力。

monster-not-polymorphed = 你沒有發生變形。

monster-scared-elbereth = { $monster } 被 Elbereth 雕刻嚇住了！

monster-teleport-near = { $monster } 憑空出現了！

mount-not-monster = 那不是你能騎乘的生物。

mount-not-tame = 那隻生物還不夠溫順，不能騎。

mount-too-far = 那隻生物離得太遠了。

mount-too-weak = 你太虛弱了，騎不上去。

no-fountain-here = 這裡沒有噴泉。

not-a-drawbridge = 那不是吊橋。

not-a-raised-drawbridge = 那座吊橋沒有升起。

not-carrying-anything = 你什麼都沒帶。

not-mounted = 你沒有騎著任何東西。

not-punished = 你沒有受罰。

not-wearing-that = 你沒有穿戴那個。

phaze-feeling-bloated = 你感到腹脹。

phaze-feeling-flabby = 你覺得自己軟趴趴的。

play-bugle = 你吹響了軍號。

play-drum = 你敲響了鼓。

play-earthquake = 整座地城都在你周圍搖晃！

play-horn-noise = 你吹出了一聲駭人而難聽的聲音。

play-magic-flute = 你奏出極其悅耳的樂音。

play-magic-harp = 你奏出極其悅耳的樂音。

play-music = 你演奏了一段樂曲。

play-nothing = 你想不出有什麼合適的東西能演奏。

polymorph-controlled = 你想變成哪種怪物？
polymorph-dismount = 你不能再騎乘你的坐騎了。

polymorph-newman-survive = 你挺過了這次變形嘗試。

polymorph-revert = 你恢復成原來的形態。

polymorph-system-shock = 你的身體顫抖著，經歷了劇烈變形！

polymorph-system-shock-fatal = 變形造成的系統衝擊殺死了你！

potion-acid-resist = 你對酸的抗性消失了！

potion-see-invisible-cursed = 你剛才好像看見了什麼。

potion-sickness-mild = 呸！這東西嚐起來像毒藥。

potion-uneasy = 你感到有些不安。

pray-angry-curse = 你感到身上的物品好像沒那麼有效了。

pray-angry-displeased = 你感到 { $god } 很不高興。

pray-angry-lose-wis = 你的智慧下降了。

pray-angry-punished = 你因為舉止不當而受到了懲罰！

pray-angry-summon = { $god } 召來了敵對怪物！

pray-angry-zap = 突然，一道閃電劈中了你！

pray-bless-weapon = 你的武器柔和地發出光芒。

pray-castle-tune = 你聽見一個聲音回蕩著：「密道口令聽起來像……」

pray-cross-altar-penalty = 你有種奇怪的禁忌感。

pray-demon-rejected = { $god } 似乎沒有被嚇退……

pray-fix-trouble = { $god } 替你解決了麻煩。

pray-gehennom-no-help = { $god } 在 Gehennom 似乎無法幫到你。

pray-golden-glow = 一道金色光輝籠罩了你。

pray-grant-intrinsic = 你感受到 { $god } 的力量。

pray-grant-spell = 神聖知識充滿了你的腦海！

pray-indifferent = { $god } 似乎無動於衷。

pray-moloch-laughter = 摩洛克嘲笑你的祈禱。

pray-pleased = 你感到 { $god } 很滿意。

pray-uncurse-all = 你感覺好像有人在幫你。

pray-undead-rebuke = 你感到自己不配。

priest-angry = 祭司發怒了！

priest-calmed = 祭司冷靜下來了。

priest-virtues-of-poverty = 祭司宣講清貧的美德。

priest-wrong-alignment = 祭司不悅地咕噥著。

punishment-applied = 你受到了懲罰！

punishment-removed = 你感到鐵球消失了。

quest-assigned = 你的任務已經指派給你了。
quest-completed = 你的任務已經完成。
quest-leader-first = { $leader } 向你致意，並衡量你的資格。
quest-leader-next = { $leader } 再次審視你，判斷你是否已經準備好。
quest-leader-assigned = { $leader } 提醒你去擊敗 { $nemesis }。
quest-leader-nemesis-dead = { $leader } 認可你帶著 { $artifact } 歸來。
quest-leader-reject = { $leader } 以「{ $reason }」為由拒絕了你。
quest-guardian = { $guardian } 警告你要忠於自己的任務。
quest-nemesis-first = { $nemesis } 擋住了你的去路。
quest-nemesis-next = { $nemesis } 仍在等待你的到來。
quest-nemesis-artifact = { $nemesis } 看見任務神器時發出怒吼。
quest-nemesis-dead = 空氣中瀰漫著 { $nemesis } 敗亡後的腐臭。
quest-expelled = 你還沒有獲准深入任務地城。
invocation-complete = 祈喚儀式成功了，一道魔法傳送門開啟了！
invocation-incomplete = 符文閃爍著，但祈喚並未完成。
invocation-missing-bell = 沒有開門鈴，儀式立刻失去了關鍵一環。
invocation-missing-candelabrum = 沒有祈喚燭臺，儀式無法成形。
invocation-needs-bell-rung = 必須先在這裡敲響開門鈴，儀式才能開始。
invocation-needs-candelabrum-ready = 祈喚燭臺必須點燃七支蠟燭才行。
invocation-items-cursed = 被詛咒的祈喚道具扭曲了整個儀式。
read-dead-book = 《死者之書》低語著墓穴般的力量。

region-fog-obscures = 一團霧氣遮蔽了你的視線！

reveal-monsters-none = 沒有可顯現的怪物。

rub-lamp-djinni = 你擦了擦燈，燈神冒了出來！

rub-lamp-nothing = 什麼也沒有發生。

rub-no-effect = 看起來什麼也沒有發生。

rub-touchstone = 你在試金石上摩擦。

rump-gets-wet = 你的屁股濕了。

sacrifice-alignment-convert = 你感到 { $god } 的力量加諸於你身上。

sacrifice-altar-convert = 祭壇被轉化了！

sacrifice-altar-reject = { $god } 拒絕了你的祭品！

sacrifice-conversion-rejected = 你聽見一聲雷鳴！

sacrifice-nothing = 你的祭品消失了！

sacrifice-unicorn-insult = { $god } 認為你的祭品是在冒犯祂。

sanctum-be-gone = "滾開，凡人！"

sanctum-desecrate = 你褻瀆了至高祭壇！

sanctum-infidel = "你竟敢闖入聖所，異教徒！"

scroll-confuse-cure = 你覺得自己不那麼混亂了。

scroll-confuse-self = 你感到一陣混亂。

scroll-destroy-armor-disenchant = 你的護甲變得沒那麼有效了！

scroll-destroy-armor-id = 你的護甲閃了一下光，然後黯淡下去。

scroll-fire-confused = 你的卷軸猛然燃燒起來！

scroll-genocide-reverse = 你製造出了一大群怪物！

scroll-genocide-reverse-self = 你感覺某種變化正降臨到自己身上。

scroll-identify-self = 你覺得自己對自己瞭若指掌……

see-item-here = 你看見這裡有一件物品。
see-items-here = 你看見這裡有好幾件物品。

scroll-cant-read-blind = 你失明時無法閱讀！

sick-deaths-door = 你已經到了死門關前。

sick-illness-severe = 你覺得病得快死了。

sit-already-riding = 你已經騎著什麼東西了。

sit-in-water = 你坐進了水裡。

sit-no-seats = 這裡沒有可以坐的東西。

sit-on-air = 坐在空氣上很好玩嗎？

sit-on-altar = 你坐在了祭壇上。

sit-on-floor = 坐在地板上很好玩嗎？

sit-on-grave = 你坐在了墓碑上。

sit-on-ice = 冰面摸起來很冷。

sit-on-lava = 岩漿燙傷了你！

sit-on-sink = 你坐在了水槽上。

sit-on-stairs = 你坐在了樓梯上。

sit-on-throne = 你感到一陣奇怪的感覺。

sit-tumble-in-place = 你在原地打了個滾。

sleepy-yawn = 你打了個哈欠。

slime-burned-away = 黏液被燒掉了！

sliming-become-slime = 你已經變成綠色史萊姆了！

sliming-limbs-oozy = 你的四肢開始變得黏糊糊的。

sliming-skin-peeling = 你的皮膚開始剝落。

sliming-turning-green = 你開始有點發綠了。

sliming-turning-into = 你正在變成綠色史萊姆！

spell-aggravation = 你覺得好像有什麼東西非常憤怒。

spell-book-full = 你已經學會太多法術了。

spell-cancellation-hit = { $target } 被一層閃爍的光芒籠罩了！

spell-cancellation-miss = 你沒打中 { $target }。

spell-cast-fail = 你沒能正確施放法術。

spell-cause-fear-none = 沒有怪物感到恐懼。

spell-charm-monster-hit = { $target } 被魅惑了！

spell-charm-monster-miss = { $target } 抵抗了！

spell-clairvoyance = 你感知到周圍的環境。

spell-confuse-monster-hit = { $target } 看起來混亂了！

spell-confuse-monster-miss = { $target } 抵抗了！

spell-confuse-monster-touch = 你的雙手開始發紅光。

spell-create-familiar = 一隻熟悉的生物出現了！

spell-create-monster = 一隻怪物出現了！

spell-cure-blindness-not-blind = 你沒有失明。

spell-cure-sickness-not-sick = 你沒有生病。

spell-curse-items = 你感到自己好像需要驅魔師。

spell-destroy-armor = 你的護甲崩解了！

spell-detect-monsters-none = 你沒有感知到任何怪物。

spell-detect-unseen-none = 你沒有感知到任何隱形之物。

spell-dig-nothing = 挖掘光束在這裡沒有作用。

spell-drain-life-hit = { $target } 突然看起來虛弱了！

spell-drain-life-miss = 你沒打中 { $target }。

spell-finger-of-death-kill = { $target } 死了！

spell-finger-of-death-resisted = { $target } 抵抗了！

spell-haste-self = 你覺得自己動作變快了。

spell-healing = 你覺得好多了。

spell-identify = 你感到自己有了自知之明……

spell-insufficient-power = 你沒有足夠的魔力施放那個法術。

spell-invisibility = 你覺得自己有點飄飄然。

spell-jumping = 你跳了起來！

spell-jumping-blocked = 有什麼東西擋住了你的跳躍。

spell-knock = 一扇門打開了！

spell-light = 一片照亮的區域環繞著你！

spell-magic-mapping = 周圍環境的地圖浮現在你眼前！

spell-need-direction = 往哪個方向？

spell-no-spellbook = 你沒有可供研讀的魔法書。

spell-polymorph-hit = { $target } 發生了變形！

ambient-court-conversation = 你聽見宮廷式的談話聲。
ambient-court-judgment = 你聽見權杖敲擊作裁決的聲音。
ambient-court-off-with-your-head = 你聽見有人喊道：「把他腦袋砍下來！」
ambient-court-beruthiel = 你聽見貝魯希爾王后的貓叫聲！
ambient-beehive-buzzing = 你聽見低沉的嗡嗡聲。
ambient-beehive-drone = 你聽見憤怒的嗡鳴聲。
ambient-beehive-bonnet = 你聽見腦子裡有蜜蜂在嗡嗡作響！
ambient-morgue-quiet = 你突然發現四周安靜得不太自然。
ambient-morgue-neck-hair = 你後頸的汗毛都豎了起來。
ambient-morgue-head-hair = 你頭上的頭髮似乎都豎了起來。
ambient-zoo-elephant = 你聽見彷彿大象踩到花生的聲音。
ambient-zoo-seal = 你聽見彷彿海豹在吠叫的聲音。
ambient-zoo-dolittle = 你聽見了杜立德醫生的聲音！
ambient-oracle-woodchucks = 你聽見有人說：「別再有土撥鼠了！」
ambient-oracle-zot = 你聽見一聲響亮的 ZOT！
ambient-vault-scrooge = 你聽見了史古基的聲音！
ambient-vault-quarterback = 你聽見四分衛在呼喊戰術。
ambient-shop-neiman-marcus = 你聽見奈曼和馬庫斯在爭吵！
spell-polymorph-miss = 你沒打中 { $target }。

spell-protection-disappears = 你的金色光輝消散了。

spell-protection-less-dense = 你的金色霧氣變淡了。

spell-remove-curse = 你感覺好像有人在幫你。

spell-restore-ability-nothing = 你感到短暫地神清氣爽。

spell-restore-ability-restored = 哇！這讓你感覺棒極了！

spell-sleep-hit = { $target } 睡著了！

spell-sleep-miss = { $target } 抵抗了！

spell-slow-monster-hit = { $target } 看起來慢了下來。

spell-slow-monster-miss = { $target } 抵抗了！

spell-stone-to-flesh-cured = 你感到身體變得靈活！

spell-stone-to-flesh-nothing = 什麼也沒有發生。

spell-summon-insects = 你召喚出昆蟲！

spell-summon-monster = 你召喚出一隻怪物！

spell-teleport-away-hit = { $target } 消失了！

spell-teleport-away-miss = { $target } 抵抗了！

spell-turn-undead-hit = { $target } 逃走了！

spell-turn-undead-miss = { $target } 抵抗了！

spell-unknown = 你不認識那個法術。

spell-weaken = { $target } 突然看起來虛弱了！

spell-wizard-lock = 一扇門鎖上了！

stairs-at-top = 你已在地城的頂層。

stairs-not-here = 這裡看不見任何樓梯。

status-blindness-end = 你又能看見了。

status-confusion-end = 你感覺沒那麼混亂了。

status-fall-asleep = 你睡著了。

status-fumble-trip = 你被什麼東西絆了一下。
status-fumbling-end = 你覺得自己沒那麼笨拙了。
status-fumbling-start = 你覺得自己笨手笨腳。
status-hallucination-end = 現在一切看起來都無聊透頂了。
status-invisibility-end = 你不再隱形了。
status-levitation-end = 你輕輕飄回了地面。
status-paralysis-end = 你又能動了。
status-paralyzed-cant-move = 你動彈不得！
status-sick-cured = 總算鬆了一口氣！
status-sick-recovered = 你感覺好多了。
status-sleepy-end = 你清醒過來了。
status-sleepy-start = 你感到睏倦。
status-speed-end = 你感覺自己慢了下來。
status-stun-end = 你覺得自己沒那麼暈了。
status-vomiting-end = 你覺得沒那麼噁心了。
status-vomiting-start = 你覺得一陣噁心。
status-wounded-legs-healed = 你的腿感覺好多了。
status-wounded-legs-start = 你的腿傷得很重！
steal-item-from-you = { $monster } 偷走了 { $item }！
steal-no-gold = { $monster } 在你身上找不到金幣。
steal-nothing-to-take = { $monster } 找不到任何可偷的東西。
steed-stops-galloping = 你的坐騎慢了下來，停住了。
steed-swims = 你的坐騎在水中划動前進。
stoning-limbs-stiffening = 你的四肢開始僵硬。
stoning-limbs-stone = 你的四肢已經變成石頭。
stoning-slowing-down = 你動作開始變慢。
stoning-turned-to-stone = 你變成了石頭。
stoning-you-are-statue = 你變成了一座雕像。
swap-no-secondary = 你沒有副手武器。
swap-success = 你交換了武器。
swap-welded = 你的武器像焊住一樣卡在手上！
swim-lava-burn = 你在岩漿裡燒成了焦炭！
swim-water-paddle = 你在水中划動前進。
temple-eerie = 你感到一陣詭異的氣氛……
temple-ghost-appears = 一個幽靈出現在你面前！
temple-shiver = 你打了個寒顫。
temple-watched = 你覺得好像有人在盯著你看。
throne-genocide = 一個聲音迴盪著：「汝當決定誰生誰死！」
throne-identify = 你感到自己有了自知之明……
throne-no-position = 你現在這個位置沒法坐上去。
throne-not-here = 這裡沒有寶座。
throne-nothing = 看起來什麼也沒有發生。
throne-vanishes = 寶座在一陣邏輯的煙霧中消失了。
throne-wish = 一個聲音迴盪著：「汝之願望已被應允！」
tip-cannot-reach = 你夠不到那個。
tip-empty = 那個容器是空的。
tip-locked = 那個容器上鎖了。

tool-bell-cursed-summon = 鈴聲召來了敵對生物！

tool-bell-cursed-undead = 鈴聲召來了不死生物！

tool-bell-no-sound = 但是聲音被悶住了。

tool-bell-opens = 有什麼東西打開了……

tool-bell-reveal = 你周圍有東西打開了……

tool-bell-ring = 鈴響了。

tool-bell-wake-nearby = 鈴聲喚醒了附近的怪物！

tool-bullwhip-crack = 你甩響了長鞭！

tool-camera-no-target = 沒有什麼可拍的。

tool-candelabrum-extinguish = 燭台上的蠟燭熄滅了。

tool-candelabrum-no-candles = 燭台上沒有裝蠟燭。

tool-candle-extinguish = 你把蠟燭熄掉了。

tool-candle-light = 你把蠟燭點亮了。

tool-cream-pie-face = 你的臉上糊滿了奶油派！

tool-dig-no-target = 這裡沒有什麼可挖的。

tool-drum-earthquake = 整座地城都在你周圍搖晃！

tool-drum-no-charges = 鼓已經沒電了。

tool-drum-thump = 你敲響了鼓。

tool-figurine-hostile = 雕像變成了敵對怪物！

tool-figurine-peaceful = 雕像變成了一隻和平怪物。

tool-figurine-tame = 雕像變成了一隻寵物！

tool-grease-empty = 潤滑油罐已經空了。

tool-grease-hands = 你的手太滑了，什麼都拿不住！

tool-grease-slip = 你上過油的 { $item } 滑掉了！

tool-horn-no-charges = 角已經沒充能了。

tool-horn-toot = 你吹出了一聲駭人而難聽的聲音。

tool-leash-no-pet = 附近沒有可拴繩的寵物。

tool-lockpick-breaks = 你的開鎖器斷掉了！

tool-magic-lamp-djinni = 一個燈神從燈裡冒了出來！

tool-magic-whistle = 你吹出一聲奇怪的口哨。

tool-mirror-self = 你照鏡子時覺得自己醜極了。

tool-no-locked-door = 那裡沒有上鎖的門。

tool-nothing-happens = 看起來什麼也沒有發生。

tool-polearm-no-target = 那裡沒有什麼可打的。

tool-saddle-no-mount = 沒有什麼東西可以裝上鞍。

tool-tin-whistle = 你吹出一聲高亢的口哨。

tool-tinning-no-corpse = 這裡沒有可以裝罐的屍體。

tool-touchstone-identify = 你把寶石在試金石上摩擦，辨認出了它們。

tool-touchstone-shatter = 寶石碎裂了！

tool-touchstone-streak = 寶石在試金石上留下了一道痕跡。

tool-towel-cursed-gunk = 你的臉上沾滿了黏糊糊的東西！

tool-towel-cursed-nothing = 你沒法把那些髒東西擦掉！

tool-towel-cursed-slimy = 你的臉摸起來黏黏的。

tool-towel-nothing = 你的臉已經很乾淨了。

tool-towel-wipe-face = 你把那團黏糊糊的東西擦掉了。

tool-unihorn-cured = 你感覺好多了！

tool-unihorn-cursed = 獨角獸角被詛咒了！

tool-unihorn-nothing = 看起來什麼也沒有發生。

tool-unlock-fail = 你沒能解開它。

tool-unlock-success = 你把它打開了。

tool-whistle-no-pets = 附近沒有任何寵物。

tunnel-blocked = 這裡沒有挖掘的空間。

turn-no-undead = 沒有不死生物可以驅離。

turn-not-clerical = 你不知道怎麼驅離不死生物。

untrap-failed = 你沒能解除陷阱。

untrap-no-trap = 你沒找到任何陷阱。

untrap-success = 你解除陷阱了！

untrap-triggered = 你觸發了陷阱！

vanquished-none = 目前還沒有消滅任何怪物。

vault-guard-disappear = 守衛消失了。

vault-guard-escort = 守衛把你護送了出去。

vault-guard-state-change = 守衛改變了姿勢。

vomiting-about-to = 你快要吐了。

vomiting-cant-think = 你腦子一團亂。

vomiting-incredibly-sick = 你覺得自己病得厲害。

vomiting-mildly-nauseated = 你感到有點反胃。

vomiting-slightly-confused = 你感到有些混亂。

vomiting-vomit = 你吐了！

wait = 時間流逝……

wand-cancel-monster = { $target } 被閃爍的光芒籠罩了！

wand-digging-miss = 挖掘光束沒命中。

wipe-cream-off = 你把臉上的奶油擦掉了。

wipe-cursed-towel = 毛巾被詛咒了！

wipe-nothing = 沒有什麼可擦掉的東西。

wizard-curse-items = 你感覺自己像是該找個驅魔人了。

wizard-detect-all = 你感知到四周的一切。

wizard-detect-monsters = 你感到彷彿有什麼東西正在注視著你。

wizard-detect-objects = 你感知到附近有物體存在。

wizard-detect-traps = 你感到附近的陷阱正在向你發出警告。

wizard-double-trouble = "雙重麻煩……"

wizard-identify-all = 你覺得自己對自己瞭若指掌……

wizard-genesis = 一隻{ $monster }出現在你身旁。

wizard-genesis-failed = 沒有什麼回應你對{ $monster }的請求。

wizard-kill = 你抹除了本層中的{ $count }隻怪物。

wizard-kill-none = 這裡沒有怪物可供抹除。

wizard-map-revealed = 你周圍環境的景象在腦海中浮現！

wizard-vague-nervous = 你隱約感到不安。

wizard-black-glow = 你注意到一陣黑色光芒籠罩著你。

wizard-aggravate = 遠處回蕩起噪音，整座地城彷彿突然甦醒了。

wizard-respawned = 延德之巫再次站了起來！

wizard-respawned-boom = 一個聲音轟然響起……

wizard-respawned-taunt = 蠢貨，你竟以為自己能{$verb}我。

wizard-steal-amulet = 延德之巫偷走了護符！

wizard-steal-invocation-tool = 延德之巫偷走了其中一件祈喚道具！

wizard-steal-quest-artifact = 延德之巫偷走了任務神器！

wizard-summon-nasties = 新的惡物憑空出現了！

wizard-taunt-laughs = {$wizard} 發出陰森的狂笑。

wizard-taunt-relinquish = 交出護符吧，{$insult}！

wizard-taunt-panic = 即便此刻，你的生命力仍在流逝，{$insult}！

wizard-taunt-last-breath = 好好珍惜你的呼吸吧，{$insult}，那會是你最後一口氣！

wizard-taunt-return = 我還會回來的。

wizard-taunt-back = 我會回來的。

wizard-taunt-general = {$malediction}，{$insult}！

amulet-feels-hot = 護符摸起來發燙！

amulet-feels-very-warm = 護符摸起來非常溫熱。

amulet-feels-warm = 護符摸起來溫熱。

wizard-where-current = 你現在位於 { $location }（絕對深度 { $absolute }）的 { $x },{ $y }。

wizard-where-special = { $level } 位於 { $location } 上。

wizard-wish = 你的願望實現了：{ $item }。

wizard-wish-adjusted = 你的願望被調整為：{ $item }。

wizard-wish-adjusted-floor = 你的願望被調整了：{ $item } 掉到了你腳邊。

wizard-wish-failed = 沒有任何東西回應你對「{ $wish }」的願望。

wizard-wish-floor = 你的願望實現了：{ $item } 掉到了你腳邊。

worm-grows = 長蟲又變長了！

worm-shrinks = 長蟲縮短了！

worn-gauntlets-power-off = 你感到虛弱了。

worn-gauntlets-power-on = 你感到更強壯了！

worn-helm-brilliance-off = 你感到自己恢復平凡了。

npc-humanoid-threatens = { $monster } 威脅你。
npc-humanoid-avoid = { $monster } 對你避之唯恐不及。
npc-humanoid-moans = { $monster } 呻吟著。
npc-humanoid-huh = { $monster } 說：「蛤？」
npc-humanoid-what = { $monster } 說：「什麼？」
npc-humanoid-eh = { $monster } 說：「咦？」
npc-humanoid-cant-see = { $monster } 說：「我看不見！」
npc-humanoid-trapped = { $monster } 說：「我被困住了！」
npc-humanoid-healing = { $monster } 索要一瓶治療藥水。
npc-humanoid-hungry = { $monster } 說：「我餓了。」
npc-humanoid-curses-orcs = { $monster } 咒罵著獸人。
npc-humanoid-mining = { $monster } 談論著採礦。
npc-humanoid-spellcraft = { $monster } 談論著法術學。
npc-humanoid-hunting = { $monster } 討論著狩獵。
npc-humanoid-gnome = { $monster } 說：「許多人進了地城，卻很少有人能回到陽光照耀的土地上。」
npc-humanoid-gnome-phase-one = { $monster } 說：「第一階段，蒐集內褲。」
npc-humanoid-gnome-phase-three = { $monster } 說：「第三階段，獲利！」
npc-humanoid-hobbit-complains = { $monster } 抱怨地城條件太惡劣。
npc-humanoid-one-ring = { $monster } 問你關於魔戒的事。
npc-humanoid-aloha = { $monster } 說：「Aloha。」
npc-humanoid-spelunker-today = { $monster } 談論《今日洞穴探險家》雜誌裡的一篇最新文章。
npc-humanoid-dungeon-exploration = { $monster } 談論地城探險。
npc-boast-gem-collection = { $monster } 炫耀自己的寶石收藏。
npc-boast-mutton = { $monster } 抱怨整天只吃羊肉。
npc-boast-fee-fie-foe-foo = { $monster } 大喊「Fee Fie Foe Foo!」然後大笑。
npc-arrest-facts-maam = { $monster } 說：「只講事實，女士。」
npc-arrest-facts-sir = { $monster } 說：「只講事實，先生。」
npc-arrest-anything-you-say = { $monster } 說：「你說的任何話都可以拿來指控你。」
npc-arrest-under-arrest = { $monster } 說：「你被捕了！」
npc-arrest-stop-law = { $monster } 說：「奉法律之名站住！」
npc-djinni-no-wishes = { $monster } 說：「抱歉，我的願望已經用完了。」
npc-djinni-free = { $monster } 說：「我自由了！」
npc-djinni-get-me-out = { $monster } 說：「把我弄出去。」
npc-djinni-disturb = { $monster } 說：「這會讓你知道別再打擾我！」
npc-cuss-curses = { $monster } 咒罵著。
npc-cuss-imprecates = { $monster } 斥責著。
npc-cuss-not-too-late = { $monster } 說：「還不算太晚。」
npc-cuss-doomed = { $monster } 說：「我們都完了。」
npc-cuss-ancestry = { $monster } 對你的祖先大肆詆毀。
npc-cuss-angel-repent = { $monster } 說：「悔改吧，如此你方可得救！」
npc-cuss-angel-insolence = { $monster } 說：「你必為你的傲慢付出代價！」
npc-cuss-angel-maker = { $monster } 說：「很快，我的孩子，你就會見到你的造物主。」
npc-cuss-angel-wrath = { $monster } 說：「天界的怒火如今已降臨於你！」
npc-cuss-angel-not-worthy = { $monster } 說：「你沒有資格尋求護符。」
npc-cuss-demon-slime = { $monster } 說：「吃下黏液去死吧！」
npc-cuss-demon-clumsy = { $monster } 說：「你是喝醉了，還是本來就這麼笨手笨腳？」
npc-cuss-demon-laughter = { $monster } 說：「饒命啊！你想笑死我嗎？」
npc-cuss-demon-amulet = { $monster } 說：「幹嘛找那個護符？你只會把它弄丟而已，笨蛋。」
npc-cuss-demon-comedian = { $monster } 說：「你應該去當喜劇演員，你的本事實在太好笑了！」
npc-cuss-demon-odor = { $monster } 說：「你有沒有考慮過遮掩一下自己的臭味？」
demon-demand-safe-passage = { $monster } 索要 { $amount } 枚 zorkmid 作為通行費。
demondemand-something = { $monster } 似乎在索求什麼。
demon-offer-all-gold = 你把所有金幣都交給了 { $monster }。
demon-offer-amount = 你給了 { $monster } { $amount } 枚 zorkmid。
demon-refuse = 你拒絕了。
demon-shortchange = 你想少給 { $monster } 一點，但手忙腳亂。
demon-vanishes-laughing = { $monster } 帶著對懦弱凡人的嘲笑消失了。
demon-scowls-vanishes = { $monster } 陰狠地瞪了你一眼，然後消失了。
demon-gets-angry = { $monster } 發怒了……
demon-good-hunting = { $monster } 說：「祝你好運，{ $honorific }。」
demon-says-something = { $monster } 說了些什麼。
demon-looks-angry = { $monster } 看起來非常憤怒。
demon-tension-building = 你感到緊張逐漸升高。
npc-spell-cantrip = { $monster } 似乎在喃喃唸著小咒。
npc-vampire-tame-craving = { $monster } 說：「我再也忍受不了這種渴望了！」
npc-vampire-tame-night-craving = { $monster } 說：「求你幫幫我，讓我滿足這不斷增長的渴望吧！」
npc-vampire-tame-weary = { $monster } 說：「我發現自己有點疲倦了。」
npc-vampire-tame-kindred-evening = { $monster } 說：「晚安，我的主人！」
npc-vampire-tame-kindred-day = { $monster } 說：「日安，我的主人。我們為何不休息呢？」
npc-vampire-tame-nightchild-craving = { $monster } 說：「夜之子啊，我再也忍受不了這種渴望了！」
npc-vampire-tame-nightchild-night-craving = { $monster } 說：「夜之子啊，求你幫幫我，讓我滿足這不斷增長的渴望吧！」
npc-vampire-tame-nightchild-weary = { $monster } 說：「夜之子啊，我發現自己有點疲倦了。」
npc-vampire-peaceful-kindred-sister = { $monster } 說：「餵食愉快，姊妹！」
npc-vampire-peaceful-kindred-brother = { $monster } 說：「餵食愉快，兄弟！」
npc-vampire-peaceful-nightchild = { $monster } 說：「聽到你的聲音真好，夜之子！」
npc-vampire-peaceful = { $monster } 說：「我只喝……藥水。」
npc-vampire-hostile-hunting-ground = { $monster } 說：「這是我的狩獵場，你竟敢在這裡遊蕩！」
npc-vampire-hostile-silver-dragon = { $monster } 說：「愚蠢！你的銀光嚇不倒我！」
npc-vampire-hostile-baby-silver-dragon = { $monster } 說：「愚蠢的小鬼！你的銀光嚇不倒我！」
npc-vampire-hostile-blood = { $monster } 說：「我要吸乾你的血！」
npc-vampire-hostile-hunt = { $monster } 說：「我會毫不留情地追殺你！」
npc-imitate-imitates = { $monster } 模仿你。
npc-rider-sandman = { $monster } 正忙著讀一本《Sandman》第 8 期。
npc-rider-war = { $monster } 說：「你以為自己是誰，War？」
npc-seduce-hello-sailor = { $monster } 說：「嗨，水手。」
npc-seduce-comes-on = { $monster } 對你獻起殷勤。
npc-seduce-cajoles = { $monster } 甜言蜜語地哄著你。
npc-nurse-put-weapon-away = { $monster } 說：「把武器收起來，免得你傷到人！」
npc-nurse-doc-cooperate = { $monster } 說：「醫生，如果你不配合，我幫不了你。」
npc-nurse-please-undress = { $monster } 說：「請脫衣服，讓我檢查一下。」
npc-nurse-take-off-shirt = { $monster } 說：「請把你的上衣脫掉。」
npc-nurse-relax = { $monster } 說：「放輕鬆，這一點也不會痛。」
npc-guard-drop-gold = { $monster } 說：「請把那些金幣放下，跟我來。」
npc-guard-follow-me = { $monster } 說：「請跟我來。」
npc-soldier-pay = { $monster } 說：「這裡的薪水爛透了！」
npc-soldier-food = { $monster } 說：「這裡的伙食連獸人都不吃！」
npc-soldier-feet = { $monster } 說：「我的腳痛死了，我整天都站著！」
npc-soldier-resistance = { $monster } 說：「抵抗是沒用的！」
npc-soldier-dog-meat = { $monster } 說：「你這條狗肉！」
npc-soldier-surrender = { $monster } 說：「投降吧！」

worn-helm-brilliance-on = 你感到神思敏銳！

write-no-marker = 你沒有魔法筆。

write-not-enough-charges = 你的筆太乾了，寫不出那個！

write-scroll-fail-daiyen-fansen = 你的筆乾掉了！

write-spellbook-fail = 魔法書扭曲了一下，然後變成空白。

write-spellbook-success = 你成功寫好了那本魔法書！

priest-not-enough-gold = 祭司要價 { $cost } 枚金幣。

priest-protection-granted = 祭司收取 { $cost } 枚金幣，賜予你庇護。

shk-welcome = { $shopkeeper } 說道：「歡迎光臨我的店，{ $honorific }。」

shk-angry-greeting = { $shopkeeper } 憤怒地瞪著你。

shk-angry-rude = { $shopkeeper } 示意自己有多討厭粗魯的顧客。

shk-angry-rude-indicates = { $shopkeeper } 明白表示，這裡一點也不歡迎粗魯的顧客。

shk-angry-non-paying = { $shopkeeper } 示意自己有多討厭不付錢的顧客。

shk-angry-non-paying-indicates = { $shopkeeper } 明白表示，這裡一點也不歡迎不付錢的顧客。

shk-follow-reminder = { $shopkeeper } 說道：「您好，{ $honorific }！您是不是忘了付帳？」

shk-follow-tap = { $shopkeeper } 輕輕拍了拍你的手臂。

shk-bill-total = { $shopkeeper } 說你的帳單一共是 { $amount } 枚金幣。

shk-bill-indicates = { $shopkeeper } 示意你的帳單一共是 { $amount } 枚金幣。

shk-debit-reminder = { $shopkeeper } 提醒你還欠 { $amount } 枚金幣。

shk-debit-indicates = { $shopkeeper } 示意你還欠 { $amount } 枚金幣。

shk-credit-reminder = { $shopkeeper } 提醒你可以使用你那 { $amount } 枚金幣的賒帳額度。

shk-credit-indicates = { $shopkeeper } 示意你還有 { $amount } 枚金幣的信用額度。

shk-robbed-greeting = { $shopkeeper } 說道：「那次搶劫我還記得清清楚楚呢，{ $honorific }。」

shk-robbed-indicates = { $shopkeeper } 示意自己對最近的搶劫事件很在意。

shk-surcharge-greeting = { $shopkeeper } 說道：「你現在要付更高的價錢了，{ $honorific }。」

shk-surcharge-indicates = { $shopkeeper } 示意你現在要付更高的價錢。

shk-business-bad = { $shopkeeper } 抱怨生意不好。

shk-business-bad-indicates = { $shopkeeper } 示意生意很差。

shk-business-good = { $shopkeeper } 說最近生意不錯。

shk-business-good-indicates = { $shopkeeper } 示意最近生意很好。

shk-shoplifters = { $shopkeeper } 抱怨商店扒手的問題。

shk-shoplifters-indicates = { $shopkeeper } 示意自己很擔心店裡的扒手。

shk-geico-pitch = { $shopkeeper } 說：「十五分鐘也許能幫你省下十五枚 zorkmid。」

shk-izchak-malls = { $shopkeeper } 說：「這些購物商場真讓我頭痛。」

shk-izchak-slow-down = { $shopkeeper } 說：「慢一點，想清楚。」

shk-izchak-one-at-a-time = { $shopkeeper } 說：「你得一件一件來。」

shk-izchak-coffee = { $shopkeeper } 說：「我不喜歡花俏咖啡……給我哥倫比亞至尊。」

shk-izchak-devteam = { $shopkeeper } 說，想讓 devteam 對任何事情達成一致都很困難。

shk-izchak-deity = { $shopkeeper } 說，侍奉自己神明的人終將興旺。

shk-izchak-high-places = { $shopkeeper } 說：「別想偷我的東西，我上頭有人！」

shk-izchak-future = { $shopkeeper } 說：「你未來很可能會需要這家店裡的東西。」

shk-izchak-valley = { $shopkeeper } 評論說，亡者之谷是一扇門戶。

npc-laugh-giggles = { $monster } 咯咯笑了起來。

npc-laugh-chuckles = { $monster } 輕聲竊笑。

npc-laugh-snickers = { $monster } 吃吃竊笑。

npc-laugh-laughs = { $monster } 笑了起來。

npc-gecko-geico-pitch = { $monster } 說：「十五分鐘也許能幫你省下十五枚 zorkmid。」

npc-mumble-incomprehensible = { $monster } 含糊不清地咕噥著。

npc-bones-rattle = { $monster } 發出嘩啦作響的聲音。

npc-shriek = { $monster } 尖聲嘶叫。

npc-bark-barks = { $monster } 汪汪叫。

npc-bark-whines = { $monster } 嗚咽起來。

npc-bark-howls = { $monster } 嚎叫起來。

npc-bark-yips = { $monster } 短促地叫了一聲。

npc-mew-mews = { $monster } 輕輕喵了一聲。

npc-mew-yowls = { $monster } 淒厲地嚎叫起來。

npc-mew-meows = { $monster } 喵喵叫。

npc-mew-purrs = { $monster } 發出呼嚕聲。

npc-growl-growls = { $monster } 發出低吼！

npc-growl-snarls = { $monster } 齜牙低吼。

npc-roar-roars = { $monster } 發出咆哮！

npc-bellow-bellows = { $monster } 發出怒吼！

npc-squeak-squeaks = { $monster } 吱吱叫。

npc-squawk-squawks = { $monster } 嘎嘎亂叫。

npc-squawk-nevermore = { $monster } 說道：「永不再來！」

npc-chirp-chirps = { $monster } 啾啾叫。

npc-hiss-hisses = { $monster } 發出嘶嘶聲！

npc-buzz-drones = { $monster } 嗡嗡作響。

npc-buzz-angry = { $monster } 憤怒地嗡嗡作響。

npc-grunt-grunts = { $monster } 哼了一聲。

npc-neigh-neighs = { $monster } 嘶鳴起來。

npc-neigh-whinnies = { $monster } 嘶鳴不已。

npc-neigh-whickers = { $monster } 發出輕快的馬鳴。

npc-were-shrieks = { $monster } 發出令人毛骨悚然的尖叫！

npc-were-howls = { $monster } 發出令人毛骨悚然的嚎叫！

npc-were-moon = { $monster } 低聲耳語，幾乎聽不清。你只勉強分辨出「月亮」兩個字。

npc-moo-moos = { $monster } 哞哞叫。

npc-wail-wails = { $monster } 哀號起來。

npc-gurgle-gurgles = { $monster } 發出咕嚕聲。

npc-burble-burbles = { $monster } 咕噥作響。

npc-trumpet-trumpets = { $monster } 發出高亢的號鳴。

npc-groan-groans = { $monster } 呻吟起來。

god-roars-suffer = 一個轟鳴的聲音咆哮道：「為你的褻瀆付出代價吧！」

god-how-dare-harm-servant = 一個轟鳴的聲音咆哮道：「你竟敢傷害我的僕從？」

god-profane-shrine = 一個轟鳴的聲音咆哮道：「你玷污了我的神殿！」
ambient-gehennom-damned = 你聽到被詛咒者的哀嚎！
ambient-gehennom-groans = 你聽到呻吟與哀號！
ambient-gehennom-laughter = 你聽到惡魔般的獰笑！
ambient-gehennom-brimstone = 你聞到硫磺味！
ambient-mines-money = 你聽到有人在數錢。
ambient-mines-register = 你聽到收銀機的叮噹聲。
ambient-mines-cart = 你聽到彷彿吃力礦車發出的聲響。
ambient-shop-shoplifters = 你聽到有人在咒罵商店扒手。
ambient-shop-register = 你聽到收銀機的叮噹聲。
ambient-shop-prices = 你聽到有人在低聲嘟囔價格。
ambient-temple-praise = 你聽到有人在讚頌 { $deity }。
ambient-temple-beseech = 你聽到有人在懇求 { $deity }。
ambient-temple-sacrifice = 你聽到有人獻上動物屍體作為祭品。
ambient-temple-donations = 你聽到有人高聲乞求捐獻。
ambient-oracle-wind = 你聽到一陣詭異的風聲。
ambient-oracle-ravings = 你聽到一陣痙攣般的胡言亂語。
ambient-oracle-snakes = 你聽到蛇的鼾聲。
ambient-barracks-honed = 你聽到有人在磨刀。
ambient-barracks-snoring = 你聽到響亮的鼾聲。
ambient-barracks-dice = 你聽到擲骰子的聲音。
ambient-barracks-macarthur = 你彷彿聽到了麥克阿瑟將軍的聲音！
ambient-swamp-mosquitoes = 你聽到蚊群的嗡嗡聲！
ambient-swamp-marsh-gas = 你聞到沼氣味！
ambient-swamp-donald-duck = 你彷彿聽到了唐老鴨的叫聲！
ambient-fountain-bubbling = 你聽到汩汩的水聲。
ambient-fountain-coins = 你聽到硬幣落入水中的叮噹聲。
ambient-fountain-naiad = 你聽到水澤仙女拍水的聲音。
ambient-fountain-soda = 你聽到像汽水噴泉般的聲響！
ambient-sink-drip = 你聽到水滴緩緩落下的聲音。
ambient-sink-gurgle = 你聽到咕嚕咕嚕的水聲。
ambient-sink-dishes = 你聽到有人在洗盤子！
ambient-vault-counting = 你聽到有人在數錢。
ambient-vault-searching = 你聽到有人在翻找什麼。
ambient-vault-footsteps = 你聽到巡邏守衛的腳步聲。
ambient-deep-crunching = 你聽到喀嚓作響的聲音。
ambient-deep-hollow = 你聽到空洞的迴響。
ambient-deep-rumble = 你聽到低沉的隆隆聲。
ambient-deep-roar = 你聽到遠處傳來的咆哮聲。
ambient-deep-digging = 你聽到有人在挖掘。
ambient-shallow-door-open = 你聽到門被打開的聲音。
ambient-shallow-door-close = 你聽到門被關上的聲音。
ambient-shallow-water = 你聽到滴水聲。
ambient-shallow-moving = 你聽到附近有人在走動。
ui-wizard-mode-disabled = 未啟用巫師模式。
ui-item-prompt-drop = 丟棄哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-wield = 揮舞哪一件？[a-zA-Z 或 - 表示空手]
ui-item-prompt-wear = 穿戴哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-take-off = 脫下哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-put-on = 戴上哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-remove = 取下哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-apply = 使用哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-ready = 準備哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-throw = 投擲哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-zap = 擊發哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-invoke = 調用哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-rub = 摩擦哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-tip = 傾倒哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-offer = 獻上哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-force-lock = 用哪一件強行撬鎖？[a-zA-Z 或 ?*]
ui-item-prompt-dip = 蘸哪一件？[a-zA-Z 或 ?*]
ui-item-prompt-dip-into = 蘸進哪一件裡？[a-zA-Z 或 ?*]
ui-item-prompt-name-item = 給哪件物品命名？[a-zA-Z 或 ?*]
ui-item-prompt-adjust-item = 調整哪件物品？[a-zA-Z 或 ?*]
ui-text-prompt-wish = 想許什麼願？
ui-text-prompt-create-monster = 要建立哪種怪物？
ui-text-prompt-teleport-level = 要傳送到地城第幾層？
ui-text-prompt-annotate-level = 給這一層加什麼註記：
ui-text-prompt-engrave = 要刻寫什麼？
ui-text-prompt-call-class = 要給哪個類別字母命名？
ui-text-prompt-call-name = 要叫它什麼？
ui-text-prompt-known-class = 查看哪個類別字母？
ui-text-prompt-cast-spell = 施放哪個法術字母？
ui-text-prompt-name-target = 命名目標（[i]物品/[m]怪物/[l]樓層）？
ui-text-prompt-name-level = 給這一層起名：
ui-text-prompt-call-monster = 要叫這隻怪物什麼？
ui-text-prompt-name-it = 要給它起什麼名字？
ui-text-prompt-assign-inventory-letter = 指定新的物品欄字母：
ui-position-prompt-travel = 要前往哪裡？
ui-position-prompt-jump = 要跳到哪裡？
ui-position-prompt-inspect = 檢查哪個位置？
ui-position-prompt-look = 查看哪個位置？
ui-position-prompt-name-monster = 給哪個位置的怪物命名？
ui-equipment-slot-weapon = 武器
ui-equipment-slot-off-hand = 副手
ui-equipment-slot-helmet = 頭盔
ui-equipment-slot-cloak = 披風
ui-equipment-slot-body-armor = 身體護甲
ui-equipment-slot-shield = 盾牌
ui-equipment-slot-gloves = 手套
ui-equipment-slot-boots = 靴子
ui-equipment-slot-shirt = 內襯
ui-equipment-slot-ring-left = 戒指（左）
ui-equipment-slot-ring-right = 戒指（右）
ui-equipment-slot-amulet = 護身符
ui-game-over-title = *** 遊戲結束 ***
ui-game-over-score-line = 分數：{ $score }
ui-game-over-cause-line = 死因：{ $cause }
ui-game-over-turns-line = 回合數：{ $turns }
ui-extcmd-desc-hash = 輸入並執行擴充命令
ui-extcmd-desc-question = 列出所有擴充命令
ui-extcmd-desc-apply = 使用一件工具
ui-extcmd-desc-cast = 施放法術
ui-extcmd-desc-chat = 與某人交談
ui-extcmd-desc-close = 關閉一扇門
ui-extcmd-desc-dip = 把一件物品浸入別的東西
ui-extcmd-desc-drop = 丟下一件物品
ui-extcmd-desc-eat = 吃東西
ui-extcmd-desc-fight = 朝一個方向攻擊
ui-extcmd-desc-glance = 查看地圖符號表示什麼
ui-extcmd-desc-inventory = 查看你的物品欄
ui-extcmd-desc-jump = 跳到一個位置
ui-extcmd-desc-kick = 踢某個方向
ui-extcmd-desc-loot = 搜刮容器或怪物
ui-extcmd-desc-look = 描述這裡有什麼
ui-extcmd-desc-open = 打開一扇門
ui-extcmd-desc-offer = 獻上祭品
ui-extcmd-desc-pickup = 撿起這裡的物品
ui-extcmd-desc-pray = 向你的神祈禱
ui-extcmd-desc-quiver = 準備要發射的彈藥
ui-extcmd-desc-quit = 離開遊戲
ui-extcmd-desc-save = 儲存遊戲
ui-extcmd-desc-search = 搜尋隱藏事物
ui-extcmd-desc-sit = 坐下
ui-extcmd-desc-throw = 投擲一件物品
ui-extcmd-desc-travel = 前往地圖上的某個位置
ui-extcmd-desc-untrap = 拆除陷阱或裝置
ui-extcmd-desc-wear = 穿上一件護甲
ui-extcmd-desc-whatis = 描述地圖符號或位置
ui-extcmd-desc-wield = 揮舞一件武器
ui-extcmd-desc-zap = 擊發一根魔杖
