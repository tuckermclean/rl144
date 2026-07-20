# rl144 — FLAVOR DRAFT v0 (SCAFFOLDING — SWAP FREELY)
#
# Every line here is a placeholder wearing the right register. IDs are
# stable: hand this to the coding agent as-is, then replace any line's
# text without touching its ID and nothing downstream breaks.
# All lines ≤78 chars. Counts match the compile's §8 budget.
# Lines marked LOCKED were canonized in session — replace with care.
# Format:  ID | text

## ============ NARRATOR CORE (60) ============
# Dry, observant, unintentionally funny. Never winks. One voice, all depths.

### movement & world
NAR_001 | You descend. The stairs do not care.
NAR_002 | The corridor continues, as corridors do.
NAR_003 | This room is mostly floor. Someone was thorough.
NAR_004 | You step over a wheel of cheese. There will be others.
NAR_005 | The dark is ahead of you. It was also behind you. It is patient.
NAR_006 | You find stairs going down. Traditionally, this is progress.
NAR_007 | Something moved. Or nothing did, loudly.
NAR_008 | The walls here have opinions. Most are about cheese.

### combat
NAR_010 | You strike the rat. It dies, which was presumably the plan.
NAR_011 | You miss. The rat watches you do it.
NAR_012 | You swing. The dark absorbs your effort without comment.
NAR_013 | It hits you. You had, technically, started it.
NAR_014 | The monster is wounded. It is looking at you differently now.
NAR_015 | You are victorious. The room is quieter and no better.
NAR_016 | Your torch burns a little faster after that. Torches keep score.

### talking
NAR_020 | You attempt diplomacy.
NAR_021 | The monster considers you.
NAR_022 | The monster declines to be moved. Its claws abstain, this once.
NAR_023 | It lands. Something in the room relaxes. Possibly you.
NAR_024 | The monster becalms. It will not trouble you again. It says nothing.
NAR_025 | You talk into the darkness. The darkness is a poor listener.
NAR_026 | That went well, by local standards.

### items
NAR_030 | You pick up a sword. It is exactly what everyone brings.
NAR_031 | You drink the potion. Your wounds close. The potion is gone forever.
NAR_032 | You take the cheese. It is historically significant.
NAR_033 | You burn the cheese. Cheese is, technically, fuel.
NAR_034 | You set down your sword. The room notices.
NAR_035 | You offer the potion. This is not what potions are for. It works.
NAR_036 | You find a coat. It is a normal coat for one normal monster.
NAR_037 | You find a towel. You cannot imagine who needs it. You will.

### light
NAR_040 | Your torch dims. This is normal and fine.
NAR_041 | You are down to embers. This is normal.
NAR_042 | You can see three tiles. Choose them well.
NAR_043 | The light is now a rumor.
NAR_044 | It is very dark. Your options are around here somewhere.

### the McGuffin (narrator's side)
NAR_050 | You pick up the amulet. It was mid-sentence.
NAR_051 | The amulet is heavier than it looks. It is also louder than it looks.
NAR_052 | Carrying it costs more light. It denies any connection.
NAR_053 | You set the amulet down. It begins introducing itself to the floor.
NAR_054 | The amulet glows, briefly. Your torch is grateful. Nobody explains.

### resurrection (LOCKED shapes)
NAR_060 | You wake beside the donkey. You are slightly damp.
NAR_061 | There are drag marks.                                    # LOCKED
NAR_062 | An item you lost below is up here now. No further information.
NAR_063 | You are alive. The donkey is looking at the horizon.

### the one mimic caption — lifetime maximum, place with care
NAR_070 | It means it about the handsome part. It also means the rest.

## ============ McGUFFIN (90) ============
# Florid, romance-poisoned, cannot walk. Carrier = "the legs."
# Registers slide ballad → adventure → buddy → no genre. No seams.

### pickup — the six words (LOCKED slot; draft below) + interruption
MCG_001 | IN THE BEGINNING, THERE WAS ME—                          # the six
MCG_002 | ...You could have knocked.
MCG_003 | No matter. No matter! We'll call that the cold open.
MCG_004 | Onward, legs. Destiny dislikes a dawdler. I read that. I wrote that.

### opening register A — bloody descent record (captive bride, wrong)
MCG_010 | Unhand— no. Hand. Keep handing. We'll workshop the rescue.
MCG_011 | Seized by a brute! It's exactly like the books. The good ones.
MCG_012 | The rats told me about you. Well. The survivors did.
MCG_013 | I shall resist you by growing fond of you. It's chapter four.

### opening register B — merciful descent record (destined, also wrong)
MCG_020 | You came. I sensed a gentleness. The rats speak highly of you.
MCG_021 | Fated. Foretold. Foreshadowed, at minimum.
MCG_022 | Four hundred years, and my hero walks in mid-monologue. Poetic.
MCG_023 | Don't speak. Or do. One of us should, and I have material.

### climb — speech re-entry attempts (each gets ~one word further)
MCG_030 | As I was saying—
MCG_031 | From the top, then: In the—
MCG_032 | Where was I. The beginning. In the beginning, there—
MCG_033 | I'll just hold my place. I'm good at holding a place.

### climb — running commentary (ad-libbing around prepared remarks)
MCG_040 | And here the hero bears me up the— stairs. Yes. As rehearsed.
MCG_041 | Note the walls. I've had four centuries with these walls. Skippable.
MCG_042 | Is that a rat you know? You know a rat. My legs know a rat.
MCG_043 | This part wasn't in the drafts. Improvising. I'm thriving.
MCG_044 | Slower on the stairs. Not for me. For the drama. Fine, for me.
MCG_045 | You breathe loudly for a legendary figure. It's humanizing. Keep it.

### mood — distress (it watched you kill)
MCG_050 | I saw that.
MCG_051 | The light seems heavier tonight. Unrelated, I'm sure.
MCG_052 | In the ballads, the sword is a last resort. This is a lot of resorts.
MCG_053 | It had a face, is all. I knew the face. Carry on.

### mood — trust (the glow, the free step)
MCG_060 | There. A little of mine. Don't mention it. Mention it a little.
MCG_061 | I've decided your footsteps have a rhythm. I've named it.
MCG_062 | Take the next step on me.
MCG_063 | You spared the one with the coat. Coats. The— you know the one.

### put-down / left alone / pick-up
MCG_070 | Setting me down. Bold. Dramatic, even. I'll allow it.
MCG_071 | A pause in the narrative. Very modern. Hurry back.
MCG_072 | Hello? Legs? ...Anyone with legs?
MCG_073 | (to the floor) He'll return. He's the returning type. I cast him.
MCG_074 | You're back. I knew it. I rehearsed knowing it.
MCG_075 | While you were gone, nothing happened. I narrated it anyway.

### sokoban coaching (cannot push; will absolutely coach)
MCG_080 | Left. No— my left. I don't have a left. Your left.
MCG_081 | I've watched eleven people fail this room. You're pacing well.
MCG_082 | Not that corner. The corner is where blocks go to be forever.
MCG_083 | Push with your back. I read that somewhere. I may have written it.

### the mimic collision (climb, D3 — McGuffin cannot leave)
MCG_090 | Oh, this material. This is old material. Sit down— DON'T sit down.
MCG_091 | The chest is quoting the ballads at my legs. Badly. Loudly. At MY legs.
MCG_092 | Stanza's wrong. The offer comes AFTER the compliment. Amateur hour.
MCG_093 | ...it does commit, though. Credit where due. Keep walking.

### the mantel — the reveal (delight; heartbreak changes owners)
MCG_100 | Another one. Just like me.
MCG_101 | LOOK at it. Look at the craftsmanship. Look at US.
MCG_102 | We'll be neighbors. Do you understand. NEIGHBORS.
MCG_103 | Set me on the left. No, the right. The light's better for both of us.
MCG_104 | (to the other) I have SO much to tell you. Starting from the top.

### keep-it ending (it never learns about the donkey; it must never)
MCG_110 | You forfeited a fortune for me. That's chapter twelve material.
MCG_111 | No mantel could hold this. Us. This. I'm keeping the speech, though.
MCG_112 | Onward, then, my legs. The sequel is unwritten. I'll narrate.

## ============ RATS — D1 (30) ============
# Want: to be echoed. Cheese: a grievance. Stages: aggrieved → wary
# → curious → something like company.

### stage 0 — aggrieved (and the cheese penalty)
RAT_001 | Cheese. He brought cheese. Four hundred years of cheese.
RAT_002 | Look around you. LOOK. Do you see a shortage of cheese.
RAT_003 | We had cheese before your grandfather had knees.
RAT_004 | The cheese age. We remember it. We remember it constantly.
RAT_005 | Say something. Anything. Not about cheese.

### stage 1 — wary (echo game landing)
RAT_010 | ...you said it back.
RAT_011 | Nobody says it back. They swing, or they cheese.
RAT_012 | Say this one back: "the dark is fine once it knows you."
RAT_013 | Again. Slower. I want to hear it in a different mouth.

### stage 2 — curious
RAT_020 | The one with the coat, on two — say things back to it. Both of it.
RAT_021 | You going down? They all go down. Say something back down there.
RAT_022 | Ask me about the tunnels. Nobody asks. I have OPINIONS.
RAT_023 | The chest on three talks pretty. Don't sit. Repeat that: don't sit.

### stage 3 — company (becalmed)
RAT_030 | You listen good. For a wall of meat.
RAT_031 | Pass through. Tell the dark I sent you. It knows me.
RAT_032 | (climb) Still walking. Still talking. Good. Both. Keep both.
RAT_033 | (climb, sees amulet) It talks MORE than you. Where'd you even.

## ============ THE COAT — D2 (20) ============
# One tall monster. It is two monsters. Engine doubles each line
# slightly out of sync; write once. Never acknowledged. No reveal.

COA_001 | Halt. I am one large monster.
COA_002 | I have always been this tall.
COA_003 | I walk in one direction at one speed, as one does.
COA_004 | State your business with me. With me.
COA_005 | (minigame prompt) ...isn't that right?          # second voice trails
COA_006 | (answered second voice) ...you heard that? You answered THAT?
COA_007 | Nobody answers the second— the echo. The echo of me. Myself.
COA_008 | I contain multitudes. A normal amount of multitudes. One.
COA_009 | (regard) You may pass on my left. Also my left.
COA_010 | (coat gift) A second coat. For. In case I get colder. Twice.
COA_011 | (uncoupled — if they choose it) We— I. We. ...We.
COA_012 | (uncoupled, after) It's warmer this way. It's louder this way. Good.
COA_013 | (climb, becalmed) Go safely. From both of— from all of me.
NAR_080 | A tall monster approaches. It is two monsters.           # LOCKED
NAR_081 | Two short monsters stand very close together. Neither explains.

## ============ THE MIMIC — D3 (30) ============
# Chest with teeth. Hunts by courtship. The want never changes;
# regard refines it. Menace and flirtation: same sentence.

### stage 0 — workmanlike
MIM_001 | You look tired. Come. Sit. Rest against me.
MIM_002 | These stairs take everything. I give it back. Sit.
MIM_003 | I've held treasures. None like you.
MIM_004 | What is a moment, between us? Sit for one.

### stage 1 — professional respect (the polite no is landing)
MIM_010 | Refused again. Graciously. You're maddening. Sit down.
MIM_011 | Most people are rude, or they sit. You're neither. What ARE you.
MIM_012 | That "no thank you" had craftsmanship. Say it again. Closer.
MIM_013 | I respect you enormously. Sit down.

### stage 2 — wistful (becalmed; it still wants to eat you, fondly)
MIM_020 | If things were different, I would still eat you. But sadly. Slowly.
MIM_021 | Go on, then. Some meals are better imagined. You're one. The best one.
MIM_022 | I'll hold this spot for you. I hold things. It's my whole nature.

### the sword custody offer
MIM_030 | Set the sword in me. I'm a chest. It's practically filing.
MIM_031 | Your arms free, your burden held, your trust... delicious. Misspoke.
MIM_032 | (sword given) It's safe inside me. Safer than you'd be. Honesty!

### the climb — material aimed at the dude, McGuffin aboard
MIM_040 | Back again, and carrying. Set it down. Set everything down. Sit.
MIM_041 | You've gone up in the world. Come down here where it's soft.
MIM_042 | (to the amulet) And YOU. Someone who appreciates a good line.
MIM_043 | (becalmed farewell) Carry your prize. I'd have carried you. Inward.

## ============ THE LOST GUY — D4 (15) ============
# Previous contractor. Lost. Naked. Fine with both. Free talk, no roll.
# Never funnier than "enough."

LOS_001 | Oh hey. You heading out? It's this way. Pretty sure.
LOS_002 | You're making a big deal out of all this, by the way.
LOS_003 | The pants are a long story. There's no ending. Like this place.
LOS_004 | It started with a wager, went to a river, and here we are. Anyway.
LOS_005 | Down THERE? No no. That's the deep end. I stay where it's shallow.
LOS_006 | I've been heading out since— what year is it. Doesn't matter.
LOS_007 | The exit moved. Exits do that. Nobody talks about it.
LOS_008 | (towel) A towel! You GET it. You're the only one who's ever got it.
LOS_009 | (towel, worn) Dignity. That's the word. Had it on the tip of me.
LOS_010 | (climb) Still here! Still heading out. Race you. Kidding. Go ahead.

## ============ THE TIRED ONES — D5 (20) ============
# Four hundred years of neighbors. Flat, weary, specific, zero cruelty.
# Talk always lands. They do NOT know about the duplicate.

TIR_001 | Take it. Please.
TIR_002 | It rehearses. All day. Through the wall. For centuries.
TIR_003 | We don't fight. Fighting takes energy. The wall took our energy.
TIR_004 | Draft thirty-one was fine. It peaked at thirty-one. Tell it that.
TIR_005 | The old opening was better. Shorter. Six words. It was perfect.
TIR_006 | It does voices. Yours, probably, by now. It did all of ours.
TIR_007 | Ask it about the tally marks. Actually don't. We'd hear about it.
TIR_008 | You want the pedestal? Straight ahead. It's very... arranged.
TIR_009 | We're not guarding it. We want to be clear. Nobody's guarding it.
TIR_010 | (pickup reaction) Mind the stairs. They're steep. Take your time. Or don't.
TIR_011 | (climb, watching) ...
TIR_012 | (climb, if spoken to) Don't jinx it. Just walk. We're fine. WALK.

## ============ OVERWORLD (30) ============

### the trainer (retired from depth 2; counts both trips separately)
TRA_001 | Rule one: kill five rats. Warms up the sword arm. Everyone does it.
TRA_002 | Bring cheese. Rats love cheese. I've been down there. Twice.
TRA_003 | Depth two, both careers. You don't forget your first depth two.
TRA_004 | Talking to monsters. Heard of it. Never saw the percentage in it.
TRA_005 | (you spare a training rat) Softhearted. The hole eats softhearted.
TRA_006 | (you talk to a training rat) That rat's on the clock, you know.
TRA_007 | (resurrection) Back already? Happens. I don't ask. You don't ask.
TRA_008 | (repeat deaths) The donkey's fond of you. Someone should be.

### the donkey (diplomacy tutorial; regard stages)
DON_001 | The donkey regards you.
DON_002 | The donkey permits an ear scratch. Your half or theirs. Unclear.
DON_003 | The donkey shifts its weight toward you. Diplomatically.
DON_004 | The donkey has becalmed. It was never not calm. Still: official.
DON_005 | The donkey stands beside you now. On purpose.

### the posting / the hole
POS_001 | NOTICE: RETRIEVAL WANTED. ONE (1) AMULET. PAYMENT ON DELIVERY.
POS_002 | The sign-up sheet holds many names. All but yours are crossed out.
POS_003 | The collector's door is shut. A card reads: NOT UNTIL IT'S IN HAND.
POS_004 | The hole does not advertise. The hole has never needed to.

## ============ LORE (50) ============

### shallow — contractor graffiti (15)
LOR_S01 | DAY 1 TIP: THE RATS DO NOT WANT THE CHEESE
LOR_S02 | DAY 2: BOUGHT MORE CHEESE
LOR_S03 | DAY 4. IT'S NOT THE CHEESE.
LOR_S04 | DAY 9: IT WAS NEVER THE CHEESE. TELL MY WIFE I SAID SOMETHING WISE.
LOR_S05 | TALK TO THEM. SOUNDS FAKE. WORKS.
LOR_S06 | THE CHEST ON THREE IS FLIRTING. DO NOT SIT.
LOR_S07 | SAT DOWN ONCE. NEVER AGAIN. (different hand:) HE SAT AGAIN
LOR_S08 | THE TALL ONE ON TWO IS OFF SOMEHOW. CAN'T PLACE IT. NICE THOUGH
LOR_S09 | IF YOU SEE A NAKED MAN HE IS FINE. HE'S BEEN FINE FOR YEARS.
LOR_S10 | LIGHT IS MONEY. SWINGING IS SPENDING. SIGNED, A BANKRUPT
LOR_S11 | FIVE FLOORS DOWN THEY DON'T EVEN FIGHT. NOBODY BELIEVES ME
LOR_S12 | WENT DOWN FOR THE AMULET. STAYING FOR NO REASON. IT'S NICE HERE
LOR_S13 | DO NOT LOOT MY CORPSE. (different hand:) LOOTED. GOOD BOOTS.
LOR_S14 | THE DONKEY KNOWS SOMETHING. THAT'S ALL. THE DONKEY KNOWS.
LOR_S15 | DAY 30: GAVE THE CHEESE TO THE DARK. THE DARK TOOK IT. UNRELATED?

### mid — the monsters' writing (15)
LOR_M01 | (mimic drafts) COMPLIMENT. OFFER SEAT. WAIT. (revised:) WAIT LONGER.
LOR_M02 | (mimic drafts) "WHAT IS A MOMENT, BETWEEN US" — keeper. use always.
LOR_M03 | (mimic drafts) do not mention the teeth early. LEARNED THIS.
LOR_M04 | (coat, two hands) provisions for one. — one. — agreed. one.
LOR_M05 | (coat, two hands) practice walking. left, then left. MY left.
LOR_M06 | (coat, two hands) if asked: tall since birth. births. BIRTH.
LOR_M07 | (rat scratch) someone said it back today. sixteen years. said it back.
LOR_M08 | (rat scratch) tunnels update: still tunnels. opinions available.
LOR_M09 | (rat scratch) the cheese ledger is FULL. no more entries. no more.
LOR_M10 | (tired ones) quiet hours: proposed again. vetoed again. by the wall.
LOR_M11 | (tired ones) draft count, this century so far: ninety. it's march.
LOR_M12 | (tired ones) it asked for notes once. ONCE. we're still not free.
LOR_M13 | (unknown hand) the dark is fine once it knows you.
LOR_M14 | (unknown hand) everything down here is somebody's roommate.
LOR_M15 | (lost guy?) shortcut to the exit: [the rest is smoothed away]

### deep — the McGuffin's drafts (20; Rule 5: no duplicate hints)
LOR_D01 | ~~HARK~~ (margin:) too much
LOR_D02 | ~~At last, my rescuer—~~ (margin:) presumptuous?
LOR_D03 | ~~O brave unmet~~ (margin:) O is doing heavy lifting
LOR_D04 | In the beginning, there was me (margin:) KEEP. OPENS STRONG.
LOR_D05 | (tally marks. many. the wall gave up before the tallies did)
LOR_D06 | ~~And lo, the door—~~ (margin:) doors don't lo
LOR_D07 | remember: pause after "me." let it land. it will land.
LOR_D08 | blocking: pedestal center. light from above if available. ASK.
LOR_D09 | if they weep: gracious. if they kneel: MORE gracious. practice.
LOR_D10 | ~~sixty stanzas~~ forty stanzas (margin:) thirty. FIRM.
LOR_D11 | the neighbors knocked again. some people don't respect craft.
LOR_D12 | year 200 revision: warmer. year 380: warmer STILL. almost there.
LOR_D13 | note: do not open with the weather. there is no weather down here.
LOR_D14 | closing line options: [a list, crossed out to the last one]
LOR_D15 | when they come, don't seem rehearsed. rehearse seeming unrehearsed.
LOR_D16 | (a cleared space. the drafts thicken toward it, then stop.)

### deep — spare slots for the speech quarry (see appendix)
LOR_D17 | ~~Long have I waited~~ (margin:) everyone says this. EVERYONE.
LOR_D18 | ~~Fear not—~~ (margin:) implies fear. flattering? risky.
LOR_D19 | stanza 12 is perfect. stanza 12 stays if everything else goes.
LOR_D20 | it will be worth it. write that down where you'll see it. here.

## ============ DEATHS & EPITAPHS (25) ============

### death messages — punchlines TO YOU (12)
DTH_001 | You died as you lived: confidently.
DTH_002 | The rat outlasts you. The rat outlasts everyone.
DTH_003 | You have died. The cheese remains.
DTH_004 | Cause of death: the plan.
DTH_005 | The dark accepts you. It accepts everything. That's its whole thing.
DTH_006 | You sat down.
DTH_007 | Your light went first. You went shortly after, technically.
DTH_008 | You swung at the wrong silence.
DTH_009 | The stairs won. The stairs were not competing.
DTH_010 | You died within sight of the exit. The exit declined to comment.
DTH_011 | (with amulet) You died mid-sentence. Not yours.
DTH_012 | (surface swing) You attacked a man beside his own mantel. Briefly.

### epitaphs — comedy FOR EVERYONE ELSE (12)
EPI_001 | Here lies a contractor. Deal fell through.               # LOCKED
EPI_002 | He made it out, actually.                                # LOCKED
EPI_003 | He died believing.                          # LOCKED (midden stone)
EPI_004 | Brought cheese.
EPI_005 | Sat down.
EPI_006 | Was told it was this way.
EPI_007 | Swung first. Ask the dark how that went.
EPI_008 | Read the fine print. Get it? He didn't either.
EPI_009 | Almost.
EPI_010 | Owed a donkey money, if you think about it.
EPI_011 | Left the block in the corner. THE corner.
EPI_012 | (greedy bot) Performed to spec. Not retained.

## ============ TITLE / CONTRACT / ENDINGS (30) ============

### title screen
TTL_001 | OSRIC THE ADEQUATE DESCENDS TO BUY OUT HALF A DONKEY
TTL_002 | (subtitle) a retrieval in five depths

### the contract (fine print; the second-playthrough scream)
CON_001 | RETRIEVAL AGREEMENT. ONE (1) AMULET. PAYMENT ON DELIVERY.
CON_002 | CONDITION: NEAR-MINT. SCRATCHES DEDUCTIBLE. LIGHT NOT PROVIDED.
CON_003 | EXCLUSIVITY: NOT GUARANTEED. SURVIVAL: SEE EXCLUSIVITY.
CON_004 | ITEM TO MATCH SPECIFICATIONS OF INVENTORY NO. 001. SIGN BELOW.
CON_005 | (a signature line. a hoofprint witness mark. yours goes beside it.)

### the mantel (LOCKED core)
COL_001 | Hm. Yes. Matches the other one.                          # LOCKED
COL_002 | There's a scratch. Depth three, I'd say. Coming out of the fee.
COL_003 | Set it on the left. It balances the mantel. Symmetry, you see.
COL_004 | (asked about the other one) It came the same way. They all do.
COL_005 | (asked if it speaks) ...The other one? No. No, it never has.
COL_006 | (regard rising) Move the case to the window? It's... possible.
COL_007 | (final talk, high) You may visit. Mornings. Don't touch the glass.

### paid ending
END_001 | The set is complete. You are paid in full.               # LOCKED
END_002 | The donkey is whole now.                                 # LOCKED
END_003 | It is larger than you remember. It was always this large. Surely.
END_004 | The dotted line is gone. You never mentioned the dotted line.
END_005 | You walk home beside it. It matches your pace. It always did.

### paid ending — epilogue (the speech gets delivered)
EPL_001 | On a mantel, in the morning light, a voice clears its throat.
EPL_002 | The other one has never said a word. It isn't going anywhere.
EPL_003 | It has started from the top.                # LOCKED (final line)

### keep-it ending
END_010 | You keep it. The fee lapses. Somewhere, a man keeps half a donkey.
END_011 | The amulet calls this the most romantic thing it has ever seen.
END_012 | It does not know what it outbid. It must never know.
END_013 | Double light, from here on. You knew the rate when you chose it.
END_014 | (amulet) Onward, my legs. The sequel is unwritten. I'll narrate.

### rat food
END_020 | You swing at a man beside his own mantel. It goes as it goes.
END_021 | The set remains incomplete. This bothers exactly one person left.

## ============ APPENDIX: THE SPEECH (the quarry) ============
# Written whole so the fragments have a mountain behind them.
# NEVER ships whole. Drafts (LOR_D) and re-entry attempts (MCG_030+)
# are cut from this. Thirty stanzas exist in fiction; this is the
# opening movement. Replace with your own quarry at will.

SPQ_001 | In the beginning, there was me.
SPQ_002 | Before the stairs were stairs. Before the dark had tenants.
SPQ_003 | I waited, as the great things wait: beautifully, and at length.
SPQ_004 | Kingdoms budded, blundered, burned. I kept my polish.
SPQ_005 | And I knew — as the deep things know — that you would come.
SPQ_006 | Not you specifically. Let me finish.
SPQ_007 | A hero. A bearer. A pair of legs with a destiny attached.
SPQ_008 | And when you came, I would say — I am saying it now — welcome.
SPQ_009 | Welcome to the last room of your old life.
SPQ_010 | Pick me up. Mind the pedestal. And from this moment—
SPQ_011 | (stanzas 12 through 30 follow. stanza 12 is perfect.)
