// ---------------------------------------------------------------------------
// Matrix digital rain background
// ---------------------------------------------------------------------------

const rainCanvas = document.getElementById('rain');
const rainCtx = rainCanvas.getContext('2d');
const GLYPHS = 'アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン0123456789';
let rainColumns = [];
const FONT_SIZE = 16;

function resizeRain() {
	rainCanvas.width = window.innerWidth;
	rainCanvas.height = window.innerHeight;
	const columnCount = Math.floor(rainCanvas.width / FONT_SIZE);
	rainColumns = new Array(columnCount).fill(0).map(() => Math.random() * -100);
}

function drawRain() {
	rainCtx.fillStyle = 'rgba(0, 0, 0, 0.08)';
	rainCtx.fillRect(0, 0, rainCanvas.width, rainCanvas.height);

	rainCtx.font = `${FONT_SIZE}px monospace`;
	for (let i = 0; i < rainColumns.length; i++) {
		const glyph = GLYPHS[Math.floor(Math.random() * GLYPHS.length)];
		const x = i * FONT_SIZE;
		const y = rainColumns[i] * FONT_SIZE;

		rainCtx.fillStyle = 'rgba(210, 255, 220, 0.85)';
		rainCtx.fillText(glyph, x, y);
		rainCtx.fillStyle = 'rgba(0, 255, 65, 0.55)';
		rainCtx.fillText(glyph, x, y + FONT_SIZE);

		if (y > rainCanvas.height && Math.random() > 0.975) {
			rainColumns[i] = 0;
		}
		rainColumns[i]++;
	}
}

window.addEventListener('resize', resizeRain);
resizeRain();
setInterval(drawRain, 50);

// ---------------------------------------------------------------------------
// Class color palette (ported from wow-gm-console's characterInfo.ts)
// ---------------------------------------------------------------------------

const CLASS_COLORS = {
	Warrior: '#C79C6E',
	Paladin: '#F58CBA',
	Hunter: '#ABD473',
	Rogue: '#FFF569',
	Priest: '#FFFFFF',
	'Death Knight': '#C41F3B',
	Shaman: '#0070DE',
	Mage: '#69CCF0',
	Warlock: '#9482C9',
	Druid: '#FF7D0A',
};
const DEFAULT_COLOR = '#00ff41';

function colorForClass(className) {
	return CLASS_COLORS[className] || DEFAULT_COLOR;
}

// Standard WoW item-quality colors, indexed by the numeric Quality field
// from item_template (0=poor .. 7=heirloom).
const QUALITY_COLORS = ['#9d9d9d', '#ffffff', '#1eff00', '#0070dd', '#a335ee', '#ff8000', '#e6cc80', '#00ccff'];

function colorForQuality(quality) {
	return QUALITY_COLORS[quality] || QUALITY_COLORS[1];
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

const state = {
	roster: new Map(), // guid -> RosterEntry
	achievementsThisSession: 0,
	levelupsThisSession: 0,
	bossKillsThisSession: 0,
	deathsThisSession: 0,
	questsThisSession: 0,
	itemsThisSession: 0,
	gatheringThisSession: 0,
	startTime: Date.now(),
};

const statusBadge = document.getElementById('status-badge');
const ticker = document.getElementById('ticker');

// ---------------------------------------------------------------------------
// Ticker
// ---------------------------------------------------------------------------

const MAX_TICKER_LINES = 150;

function pushTickerLine(html, cssClass) {
	const line = document.createElement('div');
	line.className = `event-line${cssClass ? ' ' + cssClass : ''}`;
	line.innerHTML = html;
	ticker.appendChild(line);
	while (ticker.children.length > MAX_TICKER_LINES) {
		ticker.removeChild(ticker.firstChild);
	}
}

function tag(text) {
	return `<span class="tag">[${text}]</span>`;
}

function renderEvent(evt) {
	switch (evt.type) {
		case 'wentOnline':
			pushTickerLine(`${tag('ONLINE')} ${evt.name} (${evt.race} ${evt.class}, lvl ${evt.level}) has entered the grid`, 'online');
			break;
		case 'wentOffline':
			pushTickerLine(`${tag('OFFLINE')} ${evt.name} has left the grid`, 'offline');
			break;
		case 'levelUp':
			state.levelupsThisSession++;
			updateStat('stat-levelups', state.levelupsThisSession);
			pushTickerLine(`${tag('LEVEL UP')} ${evt.name} the ${evt.class} reached level ${evt.newLevel}`, 'levelup');
			break;
		case 'zoneChanged':
			pushTickerLine(`${tag('ZONE')} ${evt.name} moved into ${evt.zoneName}`, 'zone');
			break;
		case 'achievementEarned':
			state.achievementsThisSession++;
			updateStat('stat-achievements', state.achievementsThisSession);
			pushTickerLine(`${tag('ACHIEVEMENT')} ${evt.name} earned "${evt.achievementName}"`, 'achievement');
			break;
		case 'guildActivity':
			pushTickerLine(`${tag('GUILD')} [${evt.guildName}] ${evt.description}`, 'guild');
			break;
		case 'groupFormed':
			pushTickerLine(`${tag('PARTY')} ${evt.memberNames.join(', ')} formed a party`, 'guild');
			break;
		case 'groupDisbanded':
			pushTickerLine(`${tag('PARTY')} ${evt.memberNames.join(', ')} disbanded`, 'offline');
			break;
		case 'chatExchange':
			pushTickerLine(`${tag('CHAT')} ${evt.botName}: "${evt.playerMessage}" &rarr; "${evt.botReply}"`, 'chat');
			break;
		case 'death':
			state.deathsThisSession++;
			updateStat('stat-deaths', state.deathsThisSession);
			pushTickerLine(`${tag('DEATH')} ${evt.name} has fallen in ${evt.zoneName}`, 'death');
			break;
		case 'bossKill':
			state.bossKillsThisSession++;
			updateStat('stat-bosskills', state.bossKillsThisSession);
			pushTickerLine(`${tag('BOSS KILL')} ${evt.bossName} has been defeated in ${evt.mapName}!`, 'boss-kill');
			showBossBanner(evt.bossName, evt.mapName);
			break;
		case 'questCompleted':
			state.questsThisSession++;
			updateStat('stat-quests', state.questsThisSession);
			pushTickerLine(`${tag('QUEST')} ${evt.name} completed "${evt.questName}"`, 'quest');
			break;
		case 'itemFound': {
			state.itemsThisSession++;
			updateStat('stat-items', state.itemsThisSession);
			const color = colorForQuality(evt.quality);
			pushTickerLine(
				`${tag('LOOT')} ${evt.name} found <span style="color:${color};text-shadow:0 0 6px ${color}">${evt.itemName}</span>`,
				'loot'
			);
			break;
		}
		case 'skillUp':
			state.gatheringThisSession++;
			updateStat('stat-gathering', state.gatheringThisSession);
			pushTickerLine(`${tag('GATHER')} ${evt.name}'s ${evt.skillName} reached ${evt.newValue}`, 'gather');
			break;
		default:
			pushTickerLine(`${tag('EVENT')} ${JSON.stringify(evt)}`);
	}

	let pulseVariant;
	if (evt.type === 'death') pulseVariant = 'death';
	else if (evt.type === 'itemFound') pulseVariant = 'loot';
	pulseBot(eventGuid(evt), pulseVariant);
}

let bossBannerTimeout = null;

function showBossBanner(bossName, mapName) {
	const banner = document.getElementById('boss-banner');
	const text = document.getElementById('boss-banner-text');
	text.textContent = `${bossName} DEFEATED — ${mapName}`;
	banner.hidden = false;
	// Force a reflow so re-triggering the transition works on back-to-back kills.
	void banner.offsetWidth;
	banner.classList.add('show');

	clearTimeout(bossBannerTimeout);
	bossBannerTimeout = setTimeout(() => {
		banner.classList.remove('show');
		setTimeout(() => {
			banner.hidden = true;
		}, 450);
	}, 5000);
}

function eventGuid(evt) {
	if ('guid' in evt) return evt.guid;
	return null;
}

function updateStat(id, value) {
	document.getElementById(id).textContent = value;
}

// ---------------------------------------------------------------------------
// Agent field -- four quadrants, one per continent. Each bot is placed at
// its true relative position via RosterEntry's pre-computed screenX/screenY
// (see poller::world_to_screen on the backend, derived from AzerothCore's
// own grid constants -- not a hash or a guess).
// ---------------------------------------------------------------------------

const continentEls = new Map(); // mapId -> { quadrant, radarCanvas, radarCtx, linkCanvas, linkCtx, botLayer, countEl }

for (const quadrant of document.querySelectorAll('.continent-quadrant')) {
	const mapId = Number(quadrant.dataset.map);
	const radarCanvas = quadrant.querySelector('.radar-canvas');
	const linkCanvas = quadrant.querySelector('.link-canvas');
	continentEls.set(mapId, {
		quadrant,
		radarCanvas,
		radarCtx: radarCanvas.getContext('2d'),
		linkCanvas,
		linkCtx: linkCanvas.getContext('2d'),
		botLayer: quadrant.querySelector('.bot-layer'),
		countEl: quadrant.querySelector('.continent-count'),
	});
}
const continentGrid = document.getElementById('continent-grid');

// A plain radar-style grid rather than a guessed coastline -- we have real,
// verified bot *positions* but no real map art in this pass, so the backdrop
// stays honest about that instead of faking a silhouette.
function drawRadarBackdrop(ctx, width, height) {
	ctx.clearRect(0, 0, width, height);
	const cx = width / 2;
	const cy = height / 2;

	ctx.strokeStyle = 'rgba(0, 255, 65, 0.12)';
	ctx.lineWidth = 1;
	const maxRadius = Math.sqrt(cx * cx + cy * cy);
	for (let r = maxRadius / 4; r < maxRadius; r += maxRadius / 4) {
		ctx.beginPath();
		ctx.arc(cx, cy, r, 0, Math.PI * 2);
		ctx.stroke();
	}
	ctx.beginPath();
	ctx.moveTo(cx, 0);
	ctx.lineTo(cx, height);
	ctx.moveTo(0, cy);
	ctx.lineTo(width, cy);
	ctx.stroke();

	// Echoes the real 64x64 continent grid the positions are derived from.
	ctx.strokeStyle = 'rgba(0, 255, 65, 0.05)';
	const cells = 8;
	for (let i = 1; i < cells; i++) {
		const x = (width / cells) * i;
		const y = (height / cells) * i;
		ctx.beginPath();
		ctx.moveTo(x, 0);
		ctx.lineTo(x, height);
		ctx.moveTo(0, y);
		ctx.lineTo(width, y);
		ctx.stroke();
	}
}

function resizeContinentCanvases() {
	for (const c of continentEls.values()) {
		const rect = c.quadrant.getBoundingClientRect();
		c.radarCanvas.width = rect.width;
		c.radarCanvas.height = rect.height;
		c.linkCanvas.width = rect.width;
		c.linkCanvas.height = rect.height;
		drawRadarBackdrop(c.radarCtx, rect.width, rect.height);
	}
}
window.addEventListener('resize', resizeContinentCanvases);
resizeContinentCanvases();

function renderRoster(entries) {
	const seen = new Set();
	const countByMap = new Map();

	for (const entry of entries) {
		seen.add(entry.guid);
		state.roster.set(entry.guid, entry);
		countByMap.set(entry.map, (countByMap.get(entry.map) || 0) + 1);

		const c = continentEls.get(entry.map);
		if (!c) continue; // an instance/battleground map we don't render a quadrant for

		let node = c.botLayer.querySelector(`[data-guid="${entry.guid}"]`);
		if (!node) {
			node = document.createElement('div');
			node.className = 'bot-node';
			node.dataset.guid = entry.guid;
			c.botLayer.appendChild(node);
		}

		const rect = c.quadrant.getBoundingClientRect();
		node.style.left = `${entry.screenX * rect.width}px`;
		node.style.top = `${entry.screenY * rect.height}px`;
		node.style.color = colorForClass(entry.class);
		node.style.background = colorForClass(entry.class);
	}

	// Remove nodes for bots no longer online, or that moved to an unrendered map.
	for (const [mapId, c] of continentEls) {
		for (const node of Array.from(c.botLayer.children)) {
			const guid = Number(node.dataset.guid);
			const entry = state.roster.get(guid);
			if (!seen.has(guid) || !entry || entry.map !== mapId) {
				node.remove();
			}
		}
	}
	for (const guid of Array.from(state.roster.keys())) {
		if (!seen.has(guid)) state.roster.delete(guid);
	}

	for (const [mapId, c] of continentEls) {
		c.countEl.textContent = `(${countByMap.get(mapId) || 0})`;
	}

	updateStats(entries);
	updateLegend(entries);
	drawLinks(entries);
}

function pulseBot(guid, variant) {
	if (guid == null) return;
	const node = continentGrid.querySelector(`[data-guid="${guid}"]`);
	if (!node) return;
	const classes = variant ? ['pulse', `pulse-${variant}`] : ['pulse'];
	node.classList.add(...classes);
	setTimeout(() => node.classList.remove(...classes), 900);
}

function drawLinks(entries) {
	for (const c of continentEls.values()) {
		c.linkCtx.clearRect(0, 0, c.linkCanvas.width, c.linkCanvas.height);
		c.linkCtx.strokeStyle = 'rgba(0, 229, 255, 0.45)';
		c.linkCtx.lineWidth = 1;
	}

	const groups = new Map();
	for (const entry of entries) {
		if (entry.groupId == null) continue;
		if (!groups.has(entry.groupId)) groups.set(entry.groupId, []);
		groups.get(entry.groupId).push(entry);
	}

	for (const members of groups.values()) {
		// Group members should all share a map in practice; group by map
		// defensively rather than assume it, since nothing else here does.
		const byMap = new Map();
		for (const m of members) {
			if (!byMap.has(m.map)) byMap.set(m.map, []);
			byMap.get(m.map).push(m);
		}
		for (const [mapId, mapMembers] of byMap) {
			const c = continentEls.get(mapId);
			if (!c) continue;
			for (let i = 0; i < mapMembers.length; i++) {
				for (let j = i + 1; j < mapMembers.length; j++) {
					const a = mapMembers[i];
					const b = mapMembers[j];
					c.linkCtx.beginPath();
					c.linkCtx.moveTo(a.screenX * c.linkCanvas.width, a.screenY * c.linkCanvas.height);
					c.linkCtx.lineTo(b.screenX * c.linkCanvas.width, b.screenY * c.linkCanvas.height);
					c.linkCtx.stroke();
				}
			}
		}
	}
}

// ---------------------------------------------------------------------------
// Bot hover tooltip -- a themed replacement for the native title tooltip,
// so hovering an agent reads like part of the console instead of a plain
// OS popup. Delegated on #continent-grid rather than per-node, since
// bot-node divs are created/destroyed as bots go online/offline.
// ---------------------------------------------------------------------------

const botTooltip = document.getElementById('bot-tooltip');
const botTooltipName = botTooltip.querySelector('.bot-tooltip-name');
const botTooltipClass = botTooltip.querySelector('.bot-tooltip-class');
const botTooltipZone = botTooltip.querySelector('.bot-tooltip-zone');

function showBotTooltip(guid, clientX, clientY) {
	const entry = state.roster.get(guid);
	if (!entry) {
		hideBotTooltip();
		return;
	}

	botTooltipName.textContent = entry.name;
	botTooltipName.style.color = colorForClass(entry.class);
	botTooltipClass.textContent = `${entry.race} ${entry.class} — Level ${entry.level}`;
	botTooltipZone.textContent = `${entry.zoneName}, ${entry.mapName}`;
	botTooltip.hidden = false;

	const offset = 16;
	const rect = botTooltip.getBoundingClientRect();
	let left = clientX + offset;
	let top = clientY + offset;
	if (left + rect.width > window.innerWidth - 8) left = clientX - rect.width - offset;
	if (top + rect.height > window.innerHeight - 8) top = clientY - rect.height - offset;
	botTooltip.style.left = `${Math.max(8, left)}px`;
	botTooltip.style.top = `${Math.max(8, top)}px`;
}

function hideBotTooltip() {
	botTooltip.hidden = true;
}

continentGrid.addEventListener('mousemove', (e) => {
	const node = e.target.closest('.bot-node');
	if (!node) {
		hideBotTooltip();
		return;
	}
	showBotTooltip(Number(node.dataset.guid), e.clientX, e.clientY);
});
continentGrid.addEventListener('mouseleave', hideBotTooltip);

function updateStats(entries) {
	updateStat('stat-online', entries.length);
	if (entries.length > 0) {
		const avg = entries.reduce((sum, e) => sum + e.level, 0) / entries.length;
		updateStat('stat-avglevel', avg.toFixed(1));
	} else {
		updateStat('stat-avglevel', '—');
	}
}

// ---------------------------------------------------------------------------
// Legend
// ---------------------------------------------------------------------------

const legendEls = new Map(); // className -> { countEl, avgEl }

function buildLegend() {
	const legend = document.getElementById('legend');
	legend.innerHTML = '';
	for (const [className, color] of Object.entries(CLASS_COLORS)) {
		const item = document.createElement('div');
		item.className = 'legend-item';
		item.innerHTML = `
			<span class="legend-swatch" style="background:${color}"></span>
			<span class="legend-name">${className}</span>
			<span class="legend-stats"><span class="legend-count">0</span> &middot; lvl <span class="legend-avg">&mdash;</span></span>
		`;
		legend.appendChild(item);
		legendEls.set(className, {
			countEl: item.querySelector('.legend-count'),
			avgEl: item.querySelector('.legend-avg'),
		});
	}
}
buildLegend();

function updateLegend(entries) {
	const stats = new Map(); // className -> { count, levelSum }
	for (const e of entries) {
		if (!stats.has(e.class)) stats.set(e.class, { count: 0, levelSum: 0 });
		const s = stats.get(e.class);
		s.count++;
		s.levelSum += e.level;
	}

	for (const [className, els] of legendEls) {
		const s = stats.get(className);
		els.countEl.textContent = s ? s.count : 0;
		els.avgEl.textContent = s && s.count > 0 ? (s.levelSum / s.count).toFixed(1) : '—';
	}
}

// ---------------------------------------------------------------------------
// Uptime timer
// ---------------------------------------------------------------------------

setInterval(() => {
	const elapsed = Math.floor((Date.now() - state.startTime) / 1000);
	const h = String(Math.floor(elapsed / 3600)).padStart(2, '0');
	const m = String(Math.floor((elapsed % 3600) / 60)).padStart(2, '0');
	const s = String(elapsed % 60).padStart(2, '0');
	document.getElementById('stat-uptime').textContent = `${h}:${m}:${s}`;
}, 1000);

// ---------------------------------------------------------------------------
// WebSocket
// ---------------------------------------------------------------------------

function setConnected(connected) {
	statusBadge.textContent = connected ? 'LINK: ONLINE' : 'LINK: OFFLINE';
	statusBadge.className = connected ? 'connected' : 'disconnected';
}

function connect() {
	const proto = location.protocol === 'https:' ? 'wss' : 'ws';
	const ws = new WebSocket(`${proto}://${location.host}/ws`);

	ws.addEventListener('open', () => setConnected(true));
	ws.addEventListener('close', () => {
		setConnected(false);
		setTimeout(connect, 2000);
	});
	ws.addEventListener('error', () => ws.close());

	ws.addEventListener('message', (msg) => {
		let data;
		try {
			data = JSON.parse(msg.data);
		} catch {
			return;
		}

		if (data.type === 'roster') {
			renderRoster(data.entries);
		} else {
			renderEvent(data);
		}
	});
}

connect();
