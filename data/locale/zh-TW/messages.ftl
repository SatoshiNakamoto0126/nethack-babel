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
shop-stolen = 你有未付款的商品！
shop-damage = { $shopkeeper }說「你得賠償損失！」
shop-shoplift = { $shopkeeper }尖叫道：「站住，小偷！」
temple-enter = 你進入了{ $god }的神殿。
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
terrain-wall = 牆壁
terrain-closed-door = 關閉的門
terrain-open-door = 打開的門
terrain-stairs-up = 向上的樓梯
terrain-stairs-down = 向下的樓梯
terrain-fountain = 噴泉
terrain-altar = 祭壇
terrain-water = 水
terrain-lava = 岩漿
terrain-trap = 陷阱
terrain-tree = 樹
terrain-iron-bars = 鐵柵欄

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
chat-nobody-there = 那裡沒有人可以交談。

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

## ============================================================================
## 內容傳遞（謠言、神諭）
## ============================================================================

rumor-fortune-cookie = 你打開了幸運餅乾，上面寫著：「{ $rumor }」
oracle-consultation = { $text }

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
guardian-angel-appears = 一位守護天使出現在你身旁！
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

# TODO: translate
priest-not-enough-gold = The priest asks for { $cost } gold.

# TODO: translate
priest-protection-granted = The priest grants you protection for { $cost } gold.

# TODO: translate
shk-welcome = { $shopkeeper } says: "Welcome to my shop, { $honorific }."

# TODO: translate
shk-angry-greeting = { $shopkeeper } glares at you angrily.

# TODO: translate
shk-robbed-greeting = { $shopkeeper } says: "I still remember that robbery, { $honorific }."

# TODO: translate
shk-surcharge-greeting = { $shopkeeper } says: "Prices are higher for you now, { $honorific }."

# TODO: translate
god-roars-suffer = A booming voice roars: "Suffer for thy blasphemy!"

# TODO: translate
god-how-dare-harm-servant = A booming voice roars: "How darest thou harm my servant?"

# TODO: translate
god-profane-shrine = A booming voice roars: "Thou hast profaned my shrine!"
