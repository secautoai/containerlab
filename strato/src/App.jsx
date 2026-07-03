import React from 'react';
import { SETTINGS } from './settings.js';
import { VENDORS, hue, hueBg, deviceIcon, stratoMark } from './vendors.jsx';

const mono = "'IBM Plex Mono',monospace";
const grotesk = "'Space Grotesk',sans-serif";

export default class App extends React.Component {
  constructor(props) {
    super(props);
    this.VENDORS = VENDORS;
    this.state = {
      labName: 'untitled-lab', nodes: [], links: [], messages: [], input: '',
      running: false, tab: 'console', consoleId: null, conInput: '',
      configSel: null, configs: {}, checks: [], vSummary: null,
      chips: this.initialChips(), toastText: '', deployed: false, statusLabel: 'Idle',
      sessions: [],
    };
    this.consoles = {};
    this.timers = [];
    this.msgSeq = 0;
    this.spineAdded = false;
    this.chatEl = null; this.conBodyEl = null; this.conInputEl = null; this.inputEl = null;
  }

  initialChips() {
    return [
      { label: '⚡ Multi-area OSPF: core + two branches', act: 'build' },
      { label: '📥 Import my production configs → digital twin', act: 'twin' },
    ];
  }

  componentDidMount() { if (SETTINGS.accent) document.body.style.setProperty('--accent', SETTINGS.accent); }
  componentWillUnmount() { this.timers.forEach(clearTimeout); if (this.ro) this.ro.disconnect(); }
  componentDidUpdate() {
    if (this.chatEl) this.chatEl.scrollTop = this.chatEl.scrollHeight;
    if (this.conBodyEl) this.conBodyEl.scrollTop = this.conBodyEl.scrollHeight;
    if (this.sessEl) this.sessEl.scrollTop = this.sessEl.scrollHeight;
  }

  // ── script engine ──
  seq(steps) {
    const speed = SETTINGS.agentSpeed ?? 1;
    let t = 0;
    this.setState({ running: true, statusLabel: 'Agent working' });
    steps.forEach((s, i) => {
      t += s[0] / speed;
      this.timers.push(setTimeout(() => {
        s[1]();
        if (i === steps.length - 1) this.setState({ running: false, statusLabel: 'Idle', toastText: '' });
      }, t));
    });
  }
  rec(kind, text) {
    this.setState(st => ({ sessions: [...st.sessions, { ts: Date.now(), kind, text }].slice(-500) }));
  }
  addMsg(m) {
    const id = 'msg' + (++this.msgSeq);
    if (m.role === 'user') this.rec('prompt', m.text + (m.files ? ' [' + m.files.map(f => f.name).join(', ') + ']' : ''));
    else if (m.text) this.rec('agent', m.text);
    this.setState(st => ({ messages: [...st.messages, { id, ...m }] }));
    return id;
  }
  patchMsg(id, fn) {
    this.setState(st => ({ messages: st.messages.map(m => m.id === id ? fn({ ...m, steps: m.steps ? m.steps.map(x => ({ ...x })) : m.steps }) : m) }));
  }
  step(id, label) { this.patchMsg(id, m => { (m.steps = m.steps || []).push({ label, status: 'running', meta: '' }); return m; }); }
  finStep(id, meta) {
    const msg = this.state.messages.find(x => x.id === id);
    const lbl = msg && msg.steps && msg.steps.length ? msg.steps[msg.steps.length - 1].label : '';
    if (lbl) this.rec('step', lbl + (meta ? ' — ' + meta : ''));
    this.patchMsg(id, m => { const s = m.steps[m.steps.length - 1]; if (s) { s.status = 'done'; s.meta = meta || ''; } return m; });
  }
  addNode(n) { this.rec('deploy', 'Deployed ' + n.name + ' (' + this.VENDORS[n.vendor].label + ')'); this.setState(st => ({ nodes: [...st.nodes, { status: 'booting', ...n }] })); }
  nodeRun(id) { this.setState(st => ({ nodes: st.nodes.map(n => n.id === id ? { ...n, status: 'running' } : n) })); }
  addLink(l) {
    const a = this.state.nodes.find(n => n.id === l.a), b = this.state.nodes.find(n => n.id === l.b);
    this.rec('deploy', 'Wired ' + (a ? a.name : l.a) + ' ↔ ' + (b ? b.name : l.b) + ' (' + l.net + ')');
    this.setState(st => ({ links: [...st.links, { status: 'up', ...l }] }));
  }
  addCheck(c) { this.rec('check', c.status.toUpperCase() + ' · ' + c.label + ' — ' + c.detail); this.setState(st => ({ checks: [...st.checks, c] })); }
  toast(t) { this.setState({ toastText: t }); }

  // ── scenarios ──
  routeScenario(text) {
    const p = text.toLowerCase();
    if (!this.state.deployed) return this.scnBuild(text);
    if (p.includes('spine') || p.includes('arista') || p.includes('0.0.0.1')) return this.scnSpine(text);
    if (p.includes('fail') || p.includes('shut')) return this.scnFail(text);
    if (p.includes('re-run') || p.includes('validate')) return this.scnRevalidate();
    return this.scnGeneric(text);
  }

  scnBuild(text) {
    this.addMsg({ role: 'user', text });
    const a = this.addMsg({ role: 'agent', text: '', steps: [] });
    const N = (id, name, vendor, type, x, y, area) => ({ id, name, vendor, type, x, y, area });
    const nodes = [
      N('core', 'CORE-RTR', 'srl', 'router', 470, 170, 'area 0'),
      N('bre', 'BR-EAST', 'frr', 'router', 260, 380, 'area 1'),
      N('brw', 'BR-WEST', 'ios', 'router', 680, 380, 'area 2'),
      N('he', 'HOST-E', 'linux', 'host', 260, 570, null),
      N('hw', 'HOST-W', 'linux', 'host', 680, 570, null),
    ];
    const links = [
      { id: 'L1', a: 'core', b: 'bre', net: '10.0.1.0/30' },
      { id: 'L2', a: 'core', b: 'brw', net: '10.0.2.0/30' },
      { id: 'L3', a: 'bre', b: 'brw', net: '10.0.3.0/30' },
      { id: 'L4', a: 'bre', b: 'he', net: '10.1.0.0/24' },
      { id: 'L5', a: 'brw', b: 'hw', net: '10.2.0.0/24' },
    ];
    const steps = [
      [200, () => this.step(a, 'Parsing intent')],
      [700, () => { this.finStep(a, '0.6s'); this.step(a, 'Designing topology'); this.toast('Agent is designing the topology…'); }],
      [900, () => {
        this.finStep(a, '5 devices');
        this.patchMsg(a, m => { m.text = 'Multi-area OSPF it is — CORE-RTR (Nokia SR Linux) as the area 0 backbone, BR-EAST (FRR) in area 1, BR-WEST (Cisco IOL) in area 2, with an inter-branch backup path and one host per branch.'; return m; });
        this.step(a, 'Generating per-vendor configs');
        this.setState({ labName: 'ospf-multiarea-01' });
      }],
      [1100, () => { this.finStep(a, '5 files'); this.setState({ configs: this.buildConfigs(false), configSel: 'core' }); this.step(a, 'Deploying lab'); this.toast('Deploying containers…'); }],
      ...nodes.map((n, i) => [i === 0 ? 350 : 330, () => this.addNode(n)]),
      ...links.map((l, i) => [i === 0 ? 380 : 220, () => this.addLink(l)]),
      [420, () => { this.setState(st => ({ nodes: st.nodes.map(n => ({ ...n, status: 'running' })), deployed: true })); this.finStep(a, '48s'); this.step(a, 'Validating'); this.toast('Running validation checks…'); this.setState({ checks: [] }); }],
      [500, () => this.addCheck({ label: 'OSPF adjacencies', status: 'pass', detail: '4/4 neighbors FULL — core↔bre, core↔brw, bre↔brw, DR elections ok' })],
      [420, () => this.addCheck({ label: 'Inter-area routes', status: 'pass', detail: 'O IA 10.1.0.0/24 and 10.2.0.0/24 present on all routers' })],
      [420, () => this.addCheck({ label: 'End-to-end reachability', status: 'pass', detail: 'ping HOST-E → HOST-W (10.2.0.10): 5/5, avg 1.8 ms' })],
      [420, () => this.addCheck({ label: 'Config drift', status: 'pass', detail: 'running-config matches intended config on 5/5 devices' })],
      [500, () => {
        this.finStep(a, '4 checks');
        this.setState({ vSummary: { passed: 4, total: 4, when: 'just now' } });
        this.addMsg({ role: 'agent', text: 'Lab is up — 5 devices, 5 links, all validation checks passing. Click any device to SSH into its real CLI, or keep iterating below.', report: { passed: 4, total: 4 } });
        this.setState({ chips: [
          { label: '➕ Add an Arista spine, move OSPF to area 0.0.0.1', act: 'spine' },
          { label: '⚠️ Fail the CORE ↔ BR-EAST link', act: 'fail' },
        ] });
      }],
    ];
    this.seq(steps);
  }

  scnSpine(text) {
    this.addMsg({ role: 'user', text });
    const a = this.addMsg({ role: 'agent', text: '', steps: [] });
    this.spineAdded = true;
    const steps = [
      [200, () => this.step(a, 'Parsing change request')],
      [650, () => { this.finStep(a, '0.5s'); this.step(a, 'Updating design'); this.toast('Agent is updating the design…'); }],
      [800, () => {
        this.finStep(a, '+1 device');
        this.patchMsg(a, m => { m.text = 'Adding SPINE-1 (Arista cEOS) above the core and renumbering the backbone to area 0.0.0.1 — that touches CORE-RTR, BR-EAST, and BR-WEST.'; return m; });
        this.step(a, 'Rewriting configs');
      }],
      [900, () => { this.finStep(a, '4 files changed'); this.setState({ configs: this.buildConfigs(true), configSel: 'core', tab: 'configs' }); this.step(a, 'Deploying changes'); this.toast('Pushing config changes…'); }],
      [600, () => this.addNode({ id: 'spine', name: 'SPINE-1', vendor: 'eos', type: 'switch', x: 470, y: 60, area: 'area 0.0.0.1' })],
      [350, () => this.addLink({ id: 'L6', a: 'spine', b: 'core', net: '10.0.9.0/30' })],
      [400, () => {
        this.nodeRun('spine');
        this.setState(st => ({ nodes: st.nodes.map(n => n.area && n.area.startsWith('area 0') && n.id !== 'spine' ? { ...n, area: n.id === 'core' ? 'area 0.0.0.1' : n.area } : n) }));
        this.finStep(a, '12s'); this.step(a, 'Re-validating'); this.setState({ checks: [] }); this.toast('Re-running validation…');
      }],
      [480, () => this.addCheck({ label: 'OSPF adjacencies', status: 'pass', detail: '5/5 neighbors FULL — spine↔core adjacency established in area 0.0.0.1' })],
      [400, () => this.addCheck({ label: 'Backbone renumbering', status: 'pass', detail: 'area 0.0.0.1 active on CORE-RTR + SPINE-1; ABRs advertising correctly' })],
      [400, () => this.addCheck({ label: 'Inter-area routes', status: 'pass', detail: 'O IA routes intact after renumbering, no blackholes detected' })],
      [400, () => this.addCheck({ label: 'End-to-end reachability', status: 'pass', detail: 'ping HOST-E → HOST-W: 5/5, avg 1.9 ms' })],
      [450, () => {
        this.finStep(a, '4 checks');
        this.setState({ vSummary: { passed: 4, total: 4, when: 'just now' } });
        this.addMsg({ role: 'agent', text: 'Done — SPINE-1 is in, backbone is now area 0.0.0.1, and everything still validates. The config diff is highlighted in the Configs tab.', report: { passed: 4, total: 4 } });
        this.setState({ chips: [
          { label: '⚠️ Fail the CORE ↔ BR-EAST link', act: 'fail' },
          { label: '✓ Re-run validation', act: 'reval' },
        ] });
      }],
    ];
    this.seq(steps);
  }

  scnFail(text) {
    this.addMsg({ role: 'user', text });
    const a = this.addMsg({ role: 'agent', text: '', steps: [] });
    const steps = [
      [200, () => this.step(a, 'Injecting failure')],
      [700, () => {
        this.finStep(a, 'link down');
        this.setState(st => ({ links: st.links.map(l => l.id === 'L1' ? { ...l, status: 'down' } : l) }));
        this.patchMsg(a, m => { m.text = 'Shutting the CORE-RTR ↔ BR-EAST interface pair and watching reconvergence.'; return m; });
        this.step(a, 'Observing OSPF reconvergence'); this.toast('Waiting for SPF to re-run…');
      }],
      [1200, () => { this.finStep(a, '2.3s'); this.step(a, 'Re-validating'); this.setState({ checks: [] }); }],
      [450, () => this.addCheck({ label: 'Failure injected', status: 'warn', detail: 'CORE-RTR Gi0/0 ↔ BR-EAST eth1 administratively down' })],
      [420, () => this.addCheck({ label: 'OSPF reconvergence', status: 'pass', detail: 'SPF re-ran in 2.3s — BR-EAST now routes via BR-WEST (10.0.3.0/30)' })],
      [420, () => this.addCheck({ label: 'End-to-end reachability', status: 'pass', detail: 'ping HOST-E → HOST-W: 5/5, avg 3.1 ms (backup path, +1.2 ms)' })],
      [450, () => {
        this.finStep(a, '3 checks');
        this.setState({ vSummary: { passed: 2, total: 3, warn: 1, when: 'just now' }, tab: 'validation' });
        this.addMsg({ role: 'agent', text: 'Traffic survived the failure — BR-EAST reconverged onto the inter-branch backup path in 2.3 s with no packet loss. That is the resilience you designed for.', report: { passed: 2, total: 3, warn: 1 } });
        this.setState({ chips: [
          { label: '🔧 Restore the failed link', act: 'restore' },
          { label: '✓ Re-run validation', act: 'reval' },
        ] });
      }],
    ];
    this.seq(steps);
  }

  scnRestore(text) {
    this.addMsg({ role: 'user', text });
    const a = this.addMsg({ role: 'agent', text: '', steps: [] });
    this.seq([
      [200, () => this.step(a, 'Bringing interfaces up')],
      [800, () => {
        this.finStep(a, 'link up');
        this.setState(st => ({ links: st.links.map(l => l.id === 'L1' ? { ...l, status: 'up' } : l) }));
        this.step(a, 'Re-validating'); this.setState({ checks: [] });
      }],
      [500, () => this.addCheck({ label: 'OSPF adjacencies', status: 'pass', detail: 'All neighbors FULL — primary path restored' })],
      [420, () => this.addCheck({ label: 'End-to-end reachability', status: 'pass', detail: 'ping HOST-E → HOST-W: 5/5, avg 1.8 ms (primary path)' })],
      [450, () => {
        this.finStep(a, '2 checks');
        this.setState({ vSummary: { passed: 2, total: 2, when: 'just now' } });
        this.addMsg({ role: 'agent', text: 'Link restored and traffic is back on the primary path. All green.', report: { passed: 2, total: 2 } });
        this.setState({ chips: [{ label: '⚠️ Fail the CORE ↔ BR-EAST link', act: 'fail' }, { label: '➕ Add an Arista spine, move OSPF to area 0.0.0.1', act: 'spine' }] });
      }],
    ]);
  }

  scnRevalidate() {
    this.addMsg({ role: 'user', text: 'Re-run validation' });
    const a = this.addMsg({ role: 'agent', text: '', steps: [] });
    this.seq([
      [200, () => { this.step(a, 'Re-validating'); this.setState({ checks: [], tab: 'validation' }); this.toast('Running validation checks…'); }],
      [500, () => this.addCheck({ label: 'OSPF adjacencies', status: 'pass', detail: 'All neighbors FULL' })],
      [420, () => this.addCheck({ label: 'Inter-area routes', status: 'pass', detail: 'O IA routes present on all routers' })],
      [420, () => this.addCheck({ label: 'End-to-end reachability', status: 'pass', detail: 'ping HOST-E → HOST-W: 5/5' })],
      [450, () => {
        this.finStep(a, '3 checks');
        this.setState({ vSummary: { passed: 3, total: 3, when: 'just now' } });
        this.addMsg({ role: 'agent', text: 'Validation complete — 3/3 checks passing.', report: { passed: 3, total: 3 } });
      }],
    ]);
  }

  scnTwin() {
    this.addMsg({ role: 'user', text: 'Build a digital twin from my production configs.', files: [{ name: 'core-rtr.cfg' }, { name: 'br-east.cfg' }, { name: 'fw-edge.cfg' }] });
    const a = this.addMsg({ role: 'agent', text: '', steps: [] });
    this.setState({ nodes: [], links: [], checks: [], vSummary: null, deployed: false, consoleId: null });
    this.consoles = {};
    const steps = [
      [250, () => this.step(a, 'Parsing 3 device configs')],
      [900, () => { this.finStep(a, '312 lines'); this.step(a, 'Reconstructing topology'); this.toast('Inferring topology from configs…'); this.patchMsg(a, m => { m.text = 'I can see a Cisco core, an FRR branch router, and a FortiGate edge — reconstructing the wiring from interface subnets and building a twin.'; return m; }); }],
      [900, () => { this.finStep(a, '4 devices'); this.step(a, 'Deploying digital twin'); this.setState({ labName: 'prod-twin-change-4821', configs: this.buildTwinConfigs(), configSel: 'tcore' }); }],
      [400, () => this.addNode({ id: 'tfw', name: 'FW-EDGE', vendor: 'fw', type: 'firewall', x: 470, y: 110, area: null })],
      [330, () => this.addNode({ id: 'tcore', name: 'CORE-01', vendor: 'ios', type: 'router', x: 470, y: 300, area: 'area 0' })],
      [330, () => this.addNode({ id: 'tbr', name: 'BR-01', vendor: 'frr', type: 'router', x: 250, y: 470, area: 'area 1' })],
      [330, () => this.addNode({ id: 'tsrv', name: 'SRV-NET', vendor: 'linux', type: 'host', x: 690, y: 470, area: null })],
      [380, () => this.addLink({ id: 'T1', a: 'tfw', b: 'tcore', net: '172.16.0.0/30' })],
      [220, () => this.addLink({ id: 'T2', a: 'tcore', b: 'tbr', net: '172.16.1.0/30' })],
      [220, () => this.addLink({ id: 'T3', a: 'tcore', b: 'tsrv', net: '172.16.10.0/24' })],
      [420, () => { this.setState(st => ({ nodes: st.nodes.map(n => ({ ...n, status: 'running' })), deployed: true })); this.finStep(a, '39s'); this.step(a, 'Validating twin against prod'); this.setState({ checks: [] }); }],
      [500, () => this.addCheck({ label: 'Config fidelity', status: 'pass', detail: 'Twin running-configs match imported production configs 1:1' })],
      [420, () => this.addCheck({ label: 'OSPF adjacencies', status: 'pass', detail: '2/2 neighbors FULL — matches prod adjacency table' })],
      [420, () => this.addCheck({ label: 'Firewall policy', status: 'warn', detail: 'Policy 12 references address group "legacy-vpn" that matches no prefix — same as prod' })],
      [450, () => {
        this.finStep(a, '3 checks');
        this.setState({ vSummary: { passed: 2, total: 3, warn: 1, when: 'just now' } });
        this.addMsg({ role: 'agent', text: 'Your digital twin is live — an exact mirror of the imported configs. One pre-existing warning carried over from prod (unused firewall address group). Test your change here before the maintenance window.', report: { passed: 2, total: 3, warn: 1 } });
        this.setState({ chips: [{ label: '✓ Re-run validation', act: 'reval' }, { label: '⚡ Multi-area OSPF: core + two branches', act: 'build' }] });
      }],
    ];
    this.seq(steps);
  }

  scnGeneric(text) {
    this.addMsg({ role: 'user', text });
    const a = this.addMsg({ role: 'agent', text: '', steps: [] });
    this.seq([
      [250, () => this.step(a, 'Parsing intent')],
      [700, () => { this.finStep(a, '0.5s'); this.step(a, 'Applying to lab'); }],
      [900, () => { this.finStep(a, 'no topology change'); this.step(a, 'Re-validating'); this.setState({ checks: this.state.checks.length ? this.state.checks : [] }); }],
      [700, () => {
        this.finStep(a, 'ok');
        this.addMsg({ role: 'agent', text: 'Applied. In this prototype the fully-scripted flows are the suggestion chips below — try adding the Arista spine or failing a link.' });
      }],
    ]);
  }

  chipAct(act, label) {
    const clean = label.replace(/^[^ ]+ /, '');
    if (act === 'build') this.scnBuild(clean);
    else if (act === 'twin') this.scnTwin();
    else if (act === 'spine') this.scnSpine(clean);
    else if (act === 'fail') this.scnFail(clean);
    else if (act === 'restore') this.scnRestore(clean);
    else if (act === 'reval') this.scnRevalidate();
  }

  // ── configs ──
  buildConfigs(withSpine) {
    const L = (t, s) => ({ t, s: s || 'same' });
    const core = [
      L('set / system name host-name CORE-RTR'),
      L('set / interface ethernet-1/1 subinterface 0 ipv4 address 10.0.1.1/30'),
      L('set / interface ethernet-1/2 subinterface 0 ipv4 address 10.0.2.1/30'),
    ];
    if (withSpine) core.push(L('set / interface ethernet-1/3 subinterface 0 ipv4 address 10.0.9.2/30', 'add'));
    core.push(L('set / network-instance default protocols ospf instance main version ospf-v2'));
    if (withSpine) {
      core.push(L('set / network-instance default protocols ospf instance main area 0.0.0.0', 'del'));
      core.push(L('set / network-instance default protocols ospf instance main area 0.0.0.1', 'add'));
      core.push(L('set / … area 0.0.0.1 interface ethernet-1/3.0 interface-type point-to-point', 'add'));
    } else {
      core.push(L('set / network-instance default protocols ospf instance main area 0.0.0.0'));
    }
    core.push(L('set / … area interface ethernet-1/1.0 interface-type point-to-point'));
    core.push(L('set / … area interface ethernet-1/2.0 interface-type point-to-point'));
    const bre = [
      L('frr version 10.1'), L('hostname BR-EAST'), L('!'),
      L('interface eth1'), L(' ip address 10.0.1.2/30'), L('interface eth2'), L(' ip address 10.0.3.1/30'),
      L('interface eth3'), L(' ip address 10.1.0.1/24'), L('!'), L('router ospf'),
    ];
    if (withSpine) { bre.push(L(' network 10.0.1.0/30 area 0.0.0.0', 'del')); bre.push(L(' network 10.0.1.0/30 area 0.0.0.1', 'add')); }
    else bre.push(L(' network 10.0.1.0/30 area 0.0.0.0'));
    bre.push(L(' network 10.0.3.0/30 area 1', 'same'));
    bre.push(L(' network 10.1.0.0/24 area 1'));
    const brw = [
      L('hostname BR-WEST'), L('!'), L('interface GigabitEthernet0/0'), L(' ip address 10.0.2.2 255.255.255.252'),
      L('interface GigabitEthernet0/1'), L(' ip address 10.0.3.2 255.255.255.252'),
      L('interface GigabitEthernet0/2'), L(' ip address 10.2.0.1 255.255.255.0'), L('!'), L('router ospf 1'),
    ];
    if (withSpine) { brw.push(L(' network 10.0.2.0 0.0.0.3 area 0', 'del')); brw.push(L(' network 10.0.2.0 0.0.0.3 area 0.0.0.1', 'add')); }
    else brw.push(L(' network 10.0.2.0 0.0.0.3 area 0'));
    brw.push(L(' network 10.0.3.0 0.0.0.3 area 2')); brw.push(L(' network 10.2.0.0 0.0.255.255 area 2'));
    const cfg = {
      core: { lines: core, diff: withSpine },
      bre: { lines: bre, diff: withSpine },
      brw: { lines: brw, diff: withSpine },
      he: { lines: [L('auto eth0'), L('iface eth0 inet static'), L('  address 10.1.0.10/24'), L('  gateway 10.1.0.1')], diff: false },
      hw: { lines: [L('auto eth0'), L('iface eth0 inet static'), L('  address 10.2.0.10/24'), L('  gateway 10.2.0.1')], diff: false },
    };
    if (withSpine) cfg.spine = { lines: [
      L('hostname SPINE-1', 'add'), L('!', 'add'), L('interface Ethernet1', 'add'), L('   no switchport', 'add'),
      L('   ip address 10.0.9.1/30', 'add'), L('!', 'add'), L('router ospf 1', 'add'),
      L('   network 10.0.9.0/30 area 0.0.0.1', 'add'), L('   passive-interface default', 'add'), L('   no passive-interface Ethernet1', 'add'),
    ], diff: true };
    return cfg;
  }
  buildTwinConfigs() {
    const L = (t, s) => ({ t, s: s || 'same' });
    return {
      tfw: { lines: [L('config system global'), L('    set hostname FW-EDGE'), L('end'), L('config firewall policy'), L('    edit 12'), L('        set srcaddr "legacy-vpn"'), L('        set action accept'), L('    next'), L('end')], diff: false },
      tcore: { lines: [L('hostname CORE-01'), L('!'), L('interface GigabitEthernet0/0'), L(' ip address 172.16.0.2 255.255.255.252'), L('interface GigabitEthernet0/1'), L(' ip address 172.16.1.1 255.255.255.252'), L('interface GigabitEthernet0/2'), L(' ip address 172.16.10.1 255.255.255.0'), L('router ospf 1'), L(' network 172.16.0.0 0.0.255.255 area 0')], diff: false },
      tbr: { lines: [L('frr version 10.1'), L('hostname BR-01'), L('interface eth1'), L(' ip address 172.16.1.2/30'), L('router ospf'), L(' network 172.16.1.0/30 area 1')], diff: false },
      tsrv: { lines: [L('auto eth0'), L('iface eth0 inet static'), L('  address 172.16.10.20/24'), L('  gateway 172.16.10.1')], diff: false },
    };
  }

  // ── console ──
  conFor(id) { if (!this.consoles[id]) this.consoles[id] = { buf: null, hist: [], hi: 0 }; return this.consoles[id]; }
  promptOf(n) {
    if (n.vendor === 'srl') return 'A:' + n.name + '# ';
    if (n.vendor === 'linux') return 'admin@' + n.name.toLowerCase() + ':~$ ';
    if (n.vendor === 'fw') return n.name + ' # ';
    return n.name + '# ';
  }
  openConsole(id) {
    const n = this.state.nodes.find(x => x.id === id); if (!n) return;
    const c = this.conFor(id);
    if (c.buf === null) {
      c.buf = ['Connecting to ' + n.name + ' (' + this.VENDORS[n.vendor].label + ')…', 'Warning: Permanently added to the list of known hosts.', 'Last login: ' + new Date().toUTCString().slice(0, 22), ''];
      this.rec('ssh', 'Opened SSH session to ' + n.name + ' (' + this.VENDORS[n.vendor].label + ')');
    }
    this.setState({ consoleId: id, tab: 'console' }, () => { if (this.conInputEl) this.conInputEl.focus(); });
  }
  subnetsOf(id) {
    return this.state.links.filter(l => l.a === id || l.b === id).map(l => ({ ...l, peerId: l.a === id ? l.b : l.a }));
  }
  reachable(fromId, toId) {
    const seen = new Set([fromId]); const q = [fromId];
    while (q.length) {
      const cur = q.shift();
      this.state.links.forEach(l => {
        if (l.status !== 'up') return;
        const nx = l.a === cur ? l.b : l.b === cur ? l.a : null;
        if (nx && !seen.has(nx)) { seen.add(nx); q.push(nx); }
      });
    }
    return seen.has(toId);
  }
  runCmd(raw) {
    const id = this.state.consoleId;
    const n = this.state.nodes.find(x => x.id === id); if (!n) return;
    const c = this.conFor(id);
    c.buf.push(this.promptOf(n) + raw);
    const cmd = raw.trim(); const p = cmd.toLowerCase();
    if (cmd) { c.hist.push(cmd); c.hi = c.hist.length; this.rec('cli', n.name + '# ' + cmd); }
    const out = (...ls) => c.buf.push(...ls);
    const subs = this.subnetsOf(id);
    if (!cmd) { /* empty */ }
    else if (p === 'clear' || p === 'cls') c.buf = [];
    else if (p === '?' || p === 'help') out('Commands: show version | show ip route [ospf] | show ip ospf neighbor | ping <host|ip> | clear');
    else if (p.startsWith('show version') || p === 'sh ver') out(this.VENDORS[n.vendor].label + ' — container image, serial STRATO-' + id.toUpperCase(), 'Uptime: 14 minutes. Managed by strato agent.');
    else if (p.startsWith('show ip ospf nei') || p.startsWith('sh ip ospf nei')) {
      if (n.type === 'host') out('% OSPF not running on this device.');
      else {
        out('Neighbor ID     Pri  State      Dead Time  Address        Interface');
        const rtr = subs.filter(s => { const peer = this.state.nodes.find(x => x.id === s.peerId); return peer && peer.type !== 'host' && s.status === 'up'; });
        if (!rtr.length) out('% No neighbors — links down or none configured.');
        rtr.forEach((s, i) => { const peer = this.state.nodes.find(x => x.id === s.peerId); out(('1.1.1.' + (i + 2)).padEnd(16) + '1    FULL/  -   00:00:36   ' + s.net.split('/')[0].replace(/\.0$/, '.' + (s.a === id ? 2 : 1)).padEnd(15) + 'eth' + (i + 1) + '  (' + peer.name + ')'); });
      }
    }
    else if (p.startsWith('show ip route') || p.startsWith('sh ip route')) {
      out('Codes: C - connected, O - OSPF, O IA - OSPF inter area', '');
      subs.forEach((s, i) => out('C     ' + s.net + ' is directly connected, eth' + (i + 1)));
      this.state.links.filter(l => l.a !== id && l.b !== id && l.status === 'up').forEach(l => {
        out('O IA  ' + l.net + ' [110/20] via ' + (subs[0] ? subs[0].net.split('/')[0].replace(/\.0$/, '.1') : '10.0.0.1') + ', eth1');
      });
    }
    else if (p.startsWith('ping')) {
      const arg = cmd.split(/\s+/)[1];
      if (!arg) out('% Usage: ping <host|ip>');
      else {
        const target = this.state.nodes.find(x => x.name.toLowerCase() === arg.toLowerCase() || arg.startsWith('10.') || arg.startsWith('172.'));
        const tid = target && target.id !== id ? target.id : (this.state.nodes.find(x => x.id !== id) || {}).id;
        const ok = tid && this.reachable(id, tid);
        out('PING ' + arg + ': 56 data bytes',
          ok ? '64 bytes: icmp_seq=1 ttl=62 time=1.84 ms' : 'Request timeout for icmp_seq 1',
          ok ? '64 bytes: icmp_seq=2 ttl=62 time=1.71 ms' : 'Request timeout for icmp_seq 2',
          '--- ' + arg + ' ping statistics ---',
          ok ? '2 packets transmitted, 2 received, 0% packet loss' : '2 packets transmitted, 0 received, 100% packet loss');
      }
    }
    else out('% Unknown command: "' + cmd + '" — try ?');
    c.buf.push('');
    if (c.buf.length > 300) c.buf = c.buf.slice(-220);
  }

  // ── link layer ──
  linkLayer() {
    const kids = [];
    this.state.links.forEach(l => {
      const a = this.state.nodes.find(n => n.id === l.a), b = this.state.nodes.find(n => n.id === l.b);
      if (!a || !b) return;
      const down = l.status === 'down';
      const len = Math.hypot(b.x - a.x, b.y - a.y);
      kids.push(
        <line
          key={l.id}
          x1={a.x} y1={a.y} x2={b.x} y2={b.y}
          stroke={down ? 'var(--red)' : 'var(--border2)'}
          strokeWidth={down ? 2 : 1.6}
          strokeDasharray={down ? '7 6' : len}
          strokeDashoffset={0}
          style={down ? {} : { strokeDasharray: len, animation: 'drawIn .6s ease forwards' }}
          pathLength={down ? undefined : 1}
        />,
      );
      if (down) {
        kids.push(
          <g key={l.id + 'x'} transform={`translate(${(a.x + b.x) / 2},${(a.y + b.y) / 2})`}>
            <circle r={9} fill="var(--bg)" stroke="var(--red)" strokeWidth={1.5} />
            <path d="M-3.5 -3.5 L3.5 3.5 M3.5 -3.5 L-3.5 3.5" stroke="var(--red)" strokeWidth={1.8} strokeLinecap="round" />
          </g>,
        );
      } else if (SETTINGS.packets ?? true) {
        const dur = (len / 130).toFixed(2) + 's';
        kids.push(
          <circle key={l.id + 'p'} r={2.8} fill="var(--accent)">
            <animateMotion dur={dur} repeatCount="indefinite" path={`M ${a.x} ${a.y} L ${b.x} ${b.y}`} />
          </circle>,
        );
      }
      const mx = (a.x + b.x) / 2, my = (a.y + b.y) / 2;
      if (!down) kids.push(
        <text key={l.id + 't'} x={mx} y={my - 7} fill="var(--muted)" fontSize={9.5} fontFamily={mono} textAnchor="middle" opacity={0.85}>{l.net}</text>,
      );
    });
    return (
      <svg style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', pointerEvents: 'none', zIndex: 2 }}>{kids}</svg>
    );
  }

  render() {
    const s = this.state;
    const V = this.VENDORS;

    const sendText = (text) => { if (!text.trim() || s.running) return; this.setState({ input: '' }); this.routeScenario(text.trim()); };

    const statusDot = s.running ? 'var(--accent)' : s.deployed ? 'var(--green)' : 'var(--muted)';
    const statusFg = s.running ? 'var(--accent)' : s.deployed ? 'var(--green)' : 'var(--muted)';
    const statusAnim = s.running ? 'pulseDot 1s ease-in-out infinite' : 'none';
    const statusLabel = s.running ? 'Agent working' : (s.deployed ? 'Lab running' : 'Idle');

    const tabs = [
      { key: 'console', label: 'Console', badge: s.consoleId ? 1 : 0 },
      { key: 'configs', label: 'Configs', badge: Object.keys(s.configs).length },
      { key: 'validation', label: 'Validation', badge: s.checks.length },
      { key: 'sessions', label: 'Sessions', badge: s.sessions.length },
    ];

    const conNode = s.nodes.find(n => n.id === s.consoleId);
    const conC = conNode ? this.conFor(conNode.id) : null;

    const configKeys = Object.keys(s.configs);
    const cfgSel = s.configSel && s.configs[s.configSel] ? s.configSel : configKeys[0];
    const cfg = cfgSel ? s.configs[cfgSel] : null;
    const cfgNode = s.nodes.find(n => n.id === cfgSel);

    const vs = s.vSummary;

    const fitTransform = (() => {
      if (!s.nodes.length) return 'none';
      const cw = s.cw || 900, ch = s.ch || 600;
      const xs = s.nodes.map(n => n.x), ys = s.nodes.map(n => n.y);
      const minX = Math.min(...xs) - 115, maxX = Math.max(...xs) + 115;
      const minY = Math.min(...ys) - 75, maxY = Math.max(...ys) + 75;
      const scale = Math.min(cw / (maxX - minX), ch / (maxY - minY), 1);
      const tx = (cw - (maxX - minX) * scale) / 2 - minX * scale;
      const ty = (ch - (maxY - minY) * scale) / 2 - minY * scale;
      return `translate(${tx.toFixed(1)}px, ${ty.toFixed(1)}px) scale(${scale.toFixed(3)})`;
    })();

    const sessionKindStyle = (kind) => ({
      prompt: { kind: 'PROMPT', fg: 'var(--accent)', bg: 'var(--accentSoft)' },
      agent: { kind: 'AGENT', fg: 'var(--violet)', bg: 'rgba(167,139,250,.14)' },
      step: { kind: 'STEP', fg: 'var(--muted)', bg: 'var(--panel)' },
      deploy: { kind: 'DEPLOY', fg: 'var(--green)', bg: 'rgba(62,207,142,.12)' },
      check: { kind: 'CHECK', fg: 'var(--amber)', bg: 'rgba(232,179,72,.12)' },
      ssh: { kind: 'SSH', fg: '#5a7ff0', bg: 'rgba(90,127,240,.13)' },
      cli: { kind: 'CLI', fg: '#5a7ff0', bg: 'rgba(90,127,240,.13)' },
    }[kind] || { kind: kind.toUpperCase(), fg: 'var(--muted)', bg: 'var(--panel)' });

    const exportSessions = () => {
      const txt = s.sessions.map(e => new Date(e.ts).toISOString() + '  [' + e.kind.toUpperCase() + ']  ' + e.text).join('\n');
      const blob = new Blob([txt], { type: 'text/plain' });
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = s.labName + '-session.log';
      a.click(); URL.revokeObjectURL(a.href);
    };

    return (
      <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', background: 'var(--bg)', color: 'var(--text)', fontFamily: "'IBM Plex Sans',system-ui,sans-serif", overflow: 'hidden', userSelect: 'none' }}>

        {/* ══ header ══ */}
        <div style={{ height: 50, flex: '0 0 50px', background: 'var(--panel)', borderBottom: '1px solid var(--border)', display: 'flex', alignItems: 'center', gap: 12, padding: '0 16px', zIndex: 20 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 9 }}>
            <div style={{ width: 28, height: 28, borderRadius: 8, background: 'linear-gradient(135deg,var(--accent),#2a8fd1)', display: 'grid', placeItems: 'center' }}>
              {stratoMark({ size: 15 })}
            </div>
            <span style={{ fontFamily: grotesk, fontWeight: 700, fontSize: 16, letterSpacing: '-.3px' }}>strato</span>
            <span style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--accent)', background: 'var(--accentSoft)', borderRadius: 5, padding: '2px 7px', letterSpacing: '.4px' }}>AI NETWORK ENGINEER</span>
          </div>
          <div style={{ width: 1, height: 20, background: 'var(--border)' }} />
          <span style={{ fontSize: 13, fontWeight: 500, color: 'var(--muted)' }}>Labs /</span>
          <span style={{ fontSize: 13, fontWeight: 600, marginLeft: -6, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', maxWidth: 220 }}>{s.labName}</span>
          <span style={{ fontSize: 11, color: 'var(--muted)', background: 'var(--panel2)', border: '1px solid var(--border)', borderRadius: 999, padding: '3px 10px', whiteSpace: 'nowrap' }}>cloud · us-east-1</span>
          <div style={{ flex: 1 }} />
          <div style={{ display: 'flex', alignItems: 'center', gap: 7, background: 'var(--panel2)', border: '1px solid var(--border)', borderRadius: 999, padding: '5px 12px' }}>
            <span style={{ width: 7, height: 7, borderRadius: '50%', background: statusDot, animation: statusAnim }} />
            <span style={{ fontSize: 12, fontWeight: 500, color: statusFg }}>{statusLabel}</span>
          </div>
          <div style={{ width: 30, height: 30, borderRadius: '50%', background: 'linear-gradient(135deg,#5a7ff0,#a78bfa)', display: 'grid', placeItems: 'center', fontSize: 11, fontWeight: 600, color: '#fff' }}>AK</div>
        </div>

        <div style={{ flex: 1, display: 'flex', overflow: 'hidden', minHeight: 0 }}>

          {/* ══ agent chat ══ */}
          <div style={{ width: 390, flex: '0 1 390px', minWidth: 270, background: 'var(--panel)', borderRight: '1px solid var(--border)', display: 'flex', flexDirection: 'column', minHeight: 0 }}>
            <div ref={(el) => { this.chatEl = el; }} style={{ flex: 1, overflowY: 'auto', padding: '18px 16px 8px', display: 'flex', flexDirection: 'column', gap: 16 }}>

              {s.messages.length === 0 && (
                <div style={{ margin: 'auto 8px', textAlign: 'center', padding: '40px 10px' }}>
                  <div style={{ width: 52, height: 52, borderRadius: 16, background: 'var(--accentSoft)', display: 'grid', placeItems: 'center', margin: '0 auto 14px' }}>
                    {stratoMark({ size: 26, inner: 'var(--accent)' })}
                  </div>
                  <div style={{ fontFamily: grotesk, fontSize: 17, fontWeight: 600, letterSpacing: '-.3px' }}>Describe a network</div>
                  <div style={{ fontSize: 12.5, color: 'var(--muted)', lineHeight: 1.6, marginTop: 6 }}>I'll design the topology, write per-vendor configs, deploy a real multi-vendor lab, and validate it — in about two minutes.</div>
                </div>
              )}

              {s.messages.map((m) => (
                <div key={m.id} style={{ display: 'flex', flexDirection: 'column', animation: 'fadeUp .25s ease' }}>
                  {m.role === 'user' && (
                    <div style={{ alignSelf: 'flex-end', maxWidth: '88%', background: 'var(--accentSoft)', border: '1px solid rgba(56,209,186,.25)', borderRadius: '14px 14px 4px 14px', padding: '10px 14px', fontSize: 13, lineHeight: 1.55, userSelect: 'text' }}>
                      {m.text}
                      {!!(m.files && m.files.length) && (
                        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginTop: 8 }}>
                          {m.files.map((f, fi) => (
                            <span key={fi} style={{ display: 'flex', alignItems: 'center', gap: 5, fontSize: 11, fontFamily: mono, background: 'var(--panel2)', border: '1px solid var(--border)', borderRadius: 6, padding: '4px 8px' }}>
                              <svg width={11} height={11} viewBox="0 0 24 24" fill="none" stroke="var(--muted)" strokeWidth={2}><path d="M13 3H6v18h12V8z" /><path d="M13 3v5h5" /></svg>
                              {f.name}
                            </span>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                  {m.role === 'agent' && (
                    <div style={{ display: 'flex', gap: 10, maxWidth: '100%' }}>
                      <div style={{ width: 26, height: 26, flex: '0 0 26px', borderRadius: 8, background: 'linear-gradient(135deg,var(--accent),#2a8fd1)', display: 'grid', placeItems: 'center', marginTop: 2 }}>
                        {stratoMark({ size: 13 })}
                      </div>
                      <div style={{ flex: 1, minWidth: 0 }}>
                        {!!m.text && <div style={{ fontSize: 13, lineHeight: 1.6, userSelect: 'text', whiteSpace: 'pre-wrap' }}>{m.text}</div>}
                        {!!(m.steps && m.steps.length) && (
                          <div style={{ display: 'flex', flexDirection: 'column', gap: 7, background: 'var(--panel2)', border: '1px solid var(--border)', borderRadius: 11, padding: '11px 13px', marginTop: 8 }}>
                            {m.steps.map((st, si) => (
                              <div key={si} style={{ display: 'flex', alignItems: 'center', gap: 9 }}>
                                {st.status === 'running' && <span style={{ width: 13, height: 13, flex: '0 0 13px', borderRadius: '50%', border: '2px solid var(--accentSoft)', borderTopColor: 'var(--accent)', animation: 'spin .7s linear infinite' }} />}
                                {st.status === 'done' && (
                                  <svg width={13} height={13} viewBox="0 0 24 24" fill="none" stroke="var(--green)" strokeWidth={2.6} strokeLinecap="round" strokeLinejoin="round" style={{ flex: '0 0 13px' }}><path d="M4 12.5 9.5 18 20 6.5" /></svg>
                                )}
                                <span style={{ fontSize: 12.5, color: st.status === 'running' ? 'var(--text)' : 'var(--muted)', fontWeight: 500 }}>{st.label}</span>
                                <span style={{ fontSize: 11, color: 'var(--muted)', marginLeft: 'auto', fontFamily: mono }}>{st.meta}</span>
                              </div>
                            ))}
                          </div>
                        )}
                        {!!m.report && (
                          <div
                            onClick={() => this.setState({ tab: 'validation' })}
                            style={{
                              display: 'flex', alignItems: 'center', gap: 10,
                              background: m.report.warn ? 'rgba(232,179,72,.08)' : 'rgba(62,207,142,.07)',
                              border: `1px solid ${m.report.warn ? 'rgba(232,179,72,.3)' : 'rgba(62,207,142,.28)'}`,
                              borderRadius: 11, padding: '11px 13px', marginTop: 8, cursor: 'pointer',
                            }}
                          >
                            <svg width={17} height={17} viewBox="0 0 24 24" fill="none" stroke={m.report.warn ? 'var(--amber)' : 'var(--green)'} strokeWidth={2} strokeLinecap="round" strokeLinejoin="round"><path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z" /></svg>
                            <div style={{ flex: 1 }}>
                              <div style={{ fontSize: 12.5, fontWeight: 600, color: m.report.warn ? 'var(--amber)' : 'var(--green)' }}>
                                {m.report.warn ? `${m.report.passed}/${m.report.total} checks passed · ${m.report.warn} warning` : `All ${m.report.total} validation checks passed`}
                              </div>
                              <div style={{ fontSize: 11.5, color: 'var(--muted)', marginTop: 1 }}>Validated on real device CLIs</div>
                            </div>
                            <span style={{ fontSize: 11, color: 'var(--muted)' }}>View →</span>
                          </div>
                        )}
                      </div>
                    </div>
                  )}
                </div>
              ))}
            </div>

            {/* suggestions + composer */}
            <div style={{ padding: '10px 14px 14px', borderTop: '1px solid var(--border)' }}>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginBottom: 10 }}>
                {s.chips.map((c, ci) => (
                  <button
                    key={ci}
                    className="chip"
                    onClick={() => this.chipAct(c.act, c.label)}
                    disabled={s.running}
                    style={{ border: '1px solid var(--border2)', background: 'var(--panel2)', color: 'var(--muted)', borderRadius: 999, padding: '5px 11px', fontSize: 11.5, cursor: 'pointer', textAlign: 'left', maxWidth: '100%', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}
                  >
                    {c.label}
                  </button>
                ))}
              </div>
              <div className="composer" style={{ display: 'flex', alignItems: 'flex-end', gap: 8, background: 'var(--panel2)', border: '1px solid var(--border2)', borderRadius: 13, padding: '9px 10px' }}>
                <button
                  title="Import device configs"
                  className="iconbtn"
                  onClick={() => { if (!s.running) this.scnTwin(); }}
                  disabled={s.running}
                  style={{ border: 'none', background: 'transparent', color: 'var(--muted)', cursor: 'pointer', width: 28, height: 28, borderRadius: 8, display: 'grid', placeItems: 'center', flex: '0 0 28px' }}
                >
                  <svg width={16} height={16} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round"><path d="M21 12.5 12.6 21a5.4 5.4 0 0 1-7.6-7.6L14 4.3a3.6 3.6 0 0 1 5.1 5.1L10.7 18a1.8 1.8 0 0 1-2.5-2.5l7.8-7.9" /></svg>
                </button>
                <textarea
                  ref={(el) => { this.inputEl = el; }}
                  value={s.input}
                  onChange={(e) => this.setState({ input: e.target.value })}
                  onKeyDown={(e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); sendText(s.input); } }}
                  placeholder={s.running ? 'Agent is working…' : s.deployed ? 'Describe a change — the agent updates every device…' : 'Describe the network you need…'}
                  rows={1}
                  disabled={s.running}
                  style={{ flex: 1, background: 'transparent', border: 'none', outline: 'none', resize: 'none', color: 'var(--text)', fontSize: 13, lineHeight: 1.5, maxHeight: 96, padding: '4px 0' }}
                />
                <button
                  onClick={() => sendText(s.input)}
                  disabled={s.running}
                  style={{ border: 'none', background: s.running || !s.input.trim() ? 'var(--border2)' : 'var(--accent)', color: '#08211d', width: 30, height: 30, borderRadius: 9, cursor: 'pointer', display: 'grid', placeItems: 'center', flex: '0 0 30px' }}
                >
                  <svg width={14} height={14} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2.4} strokeLinecap="round" strokeLinejoin="round"><path d="M12 19V5M5 12l7-7 7 7" /></svg>
                </button>
              </div>
            </div>
          </div>

          {/* ══ canvas ══ */}
          <div
            ref={(el) => {
              this.canvasEl = el;
              if (el && !this.ro && window.ResizeObserver) {
                this.ro = new ResizeObserver(() => {
                  const r = el.getBoundingClientRect();
                  if (Math.abs((this.state.cw || 0) - r.width) > 2 || Math.abs((this.state.ch || 0) - r.height) > 2) this.setState({ cw: r.width, ch: r.height });
                });
                this.ro.observe(el);
              }
            }}
            style={{ flex: '1 1 auto', position: 'relative', overflow: 'hidden', minWidth: 260, background: 'var(--bg)', backgroundImage: 'radial-gradient(#1a212c 1.2px, transparent 1.2px)', backgroundSize: '26px 26px' }}
          >
            {s.nodes.length === 0 && (
              <div style={{ position: 'absolute', inset: 0, display: 'grid', placeItems: 'center' }}>
                <div style={{ textAlign: 'center', color: 'var(--muted)' }}>
                  <svg width={44} height={44} viewBox="0 0 24 24" fill="none" stroke="var(--border2)" strokeWidth={1.4} style={{ marginBottom: 10 }}><circle cx={5.5} cy={6} r={2.5} /><circle cx={18.5} cy={6} r={2.5} /><circle cx={12} cy={18} r={2.5} /><path d="M7.5 7.5 10.5 16M16.5 7.5 13.5 16M8 6h8" /></svg>
                  <div style={{ fontSize: 13 }}>The lab you describe will build itself here.</div>
                </div>
              </div>
            )}

            <div style={{ position: 'absolute', left: 0, top: 0, width: 940, height: 660, transformOrigin: '0 0', transform: fitTransform, transition: 'transform .5s ease' }}>
              {this.linkLayer()}

              {s.nodes.map((n) => (
                <div
                  key={n.id}
                  onClick={() => this.openConsole(n.id)}
                  title="Click to SSH"
                  style={{ position: 'absolute', left: n.x, top: n.y, transform: 'translate(-50%,-50%)', cursor: 'pointer', zIndex: 5, animation: 'popIn .45s cubic-bezier(.2,1.2,.4,1)' }}
                >
                  <div
                    className="nodecard"
                    style={{
                      display: 'flex', alignItems: 'center', gap: 10, background: 'var(--panel)',
                      border: `1.5px solid ${s.consoleId === n.id ? 'var(--accent)' : 'var(--border)'}`,
                      borderRadius: 12, padding: '9px 14px 9px 9px',
                      boxShadow: s.consoleId === n.id ? '0 0 0 3px var(--accentSoft), var(--shadow)' : 'var(--shadow)',
                      minWidth: 138,
                    }}
                  >
                    <div style={{ width: 34, height: 34, borderRadius: 9, background: hueBg(V[n.vendor].hue), display: 'grid', placeItems: 'center', flex: '0 0 34px' }}>
                      {deviceIcon(n.type, 18, hue(V[n.vendor].hue))}
                    </div>
                    <div>
                      <div style={{ fontSize: 12.5, fontWeight: 600, letterSpacing: '-.1px', whiteSpace: 'nowrap', fontFamily: mono }}>{n.name}</div>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 5, marginTop: 2 }}>
                        <span style={{ width: 6, height: 6, borderRadius: '50%', background: n.status === 'running' ? 'var(--green)' : 'var(--amber)', animation: n.status === 'running' ? 'none' : 'pulseDot 1.2s ease-in-out infinite' }} />
                        <span style={{ fontSize: 10.5, color: 'var(--muted)' }}>{V[n.vendor].label}</span>
                      </div>
                    </div>
                  </div>
                  {!!n.area && (
                    <div style={{ position: 'absolute', top: -9, right: -6, fontSize: 9.5, fontWeight: 600, fontFamily: mono, color: 'var(--violet)', background: 'rgba(167,139,250,.14)', border: '1px solid rgba(167,139,250,.35)', borderRadius: 5, padding: '1px 6px' }}>{n.area}</div>
                  )}
                </div>
              ))}
            </div>

            {!!s.toastText && s.running && (
              <div style={{ position: 'absolute', top: 14, left: '50%', transform: 'translateX(-50%)', background: 'var(--panel)', border: '1px solid var(--border2)', borderRadius: 999, padding: '7px 16px', fontSize: 12, color: 'var(--muted)', boxShadow: 'var(--shadow)', zIndex: 15, display: 'flex', alignItems: 'center', gap: 8, animation: 'fadeUp .25s ease' }}>
                <span style={{ width: 11, height: 11, borderRadius: '50%', border: '2px solid var(--accentSoft)', borderTopColor: 'var(--accent)', animation: 'spin .7s linear infinite' }} />
                {s.toastText}
              </div>
            )}

            <div style={{ position: 'absolute', left: 14, bottom: 14, display: 'flex', gap: 8, zIndex: 10 }}>
              <span style={{ fontSize: 11, color: 'var(--muted)', background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 8, padding: '6px 11px', whiteSpace: 'nowrap' }}>{s.nodes.length} devices · {s.links.length} links</span>
              {s.deployed && (
                <span style={{ fontSize: 11, color: 'var(--muted)', background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 8, padding: '6px 11px', whiteSpace: 'nowrap' }}>Click a device to SSH</span>
              )}
            </div>
          </div>

          {/* ══ right panel ══ */}
          <div style={{ width: 400, flex: '0 1 400px', minWidth: 280, background: 'var(--panel)', borderLeft: '1px solid var(--border)', display: 'flex', flexDirection: 'column', minHeight: 0 }}>
            <div style={{ display: 'flex', borderBottom: '1px solid var(--border)', padding: '0 4px', flex: '0 0 auto', minWidth: 0 }}>
              {tabs.map((t) => (
                <button
                  key={t.key}
                  onClick={() => this.setState({ tab: t.key })}
                  style={{
                    border: 'none', background: 'transparent',
                    color: s.tab === t.key ? 'var(--text)' : 'var(--muted)',
                    fontSize: 12, fontWeight: 600, padding: '13px 8px 11px', cursor: 'pointer',
                    borderBottom: `2px solid ${s.tab === t.key ? 'var(--accent)' : 'transparent'}`,
                    display: 'flex', alignItems: 'center', gap: 5, minWidth: 0, flex: '0 1 auto', whiteSpace: 'nowrap', overflow: 'hidden',
                  }}
                >
                  {t.label}
                  {t.badge > 0 && (
                    <span style={{ fontSize: 10, fontWeight: 600, background: s.tab === t.key ? 'var(--accentSoft)' : 'var(--panel2)', color: s.tab === t.key ? 'var(--accent)' : 'var(--muted)', borderRadius: 999, padding: '1px 6px' }}>{t.badge}</span>
                  )}
                </button>
              ))}
            </div>

            {/* console tab */}
            {s.tab === 'console' && (conNode ? (
              <>
                <div style={{ display: 'flex', gap: 6, padding: '10px 12px 0', flexWrap: 'wrap', flex: '0 0 auto' }}>
                  {s.nodes.map((n) => (
                    <button
                      key={n.id}
                      onClick={() => this.openConsole(n.id)}
                      style={{
                        border: `1px solid ${s.consoleId === n.id ? 'var(--accent)' : 'var(--border)'}`,
                        background: s.consoleId === n.id ? 'var(--accentSoft)' : 'var(--panel2)',
                        color: s.consoleId === n.id ? 'var(--accent)' : 'var(--muted)',
                        borderRadius: 7, padding: '4px 9px', fontSize: 11, fontWeight: 600, fontFamily: mono, cursor: 'pointer',
                      }}
                    >
                      {n.name}
                    </button>
                  ))}
                </div>
                <div style={{ fontSize: 11, color: 'var(--muted)', padding: '9px 14px 7px', fontFamily: mono, flex: '0 0 auto' }}>ssh admin@{conNode.name} · {V[conNode.vendor].label}</div>
                <div
                  ref={(el) => { this.conBodyEl = el; }}
                  onClick={() => { if (this.conInputEl) this.conInputEl.focus(); }}
                  style={{ flex: 1, overflowY: 'auto', background: '#0a0c0f', borderTop: '1px solid var(--border)', padding: '12px 14px', fontFamily: mono, fontSize: 11.5, lineHeight: 1.55, color: '#cde5d8', cursor: 'text', userSelect: 'text', position: 'relative' }}
                >
                  {(conC && conC.buf ? conC.buf : []).map((t, ti) => (
                    <div key={ti} style={{ whiteSpace: 'pre-wrap', minHeight: '1em' }}>{t}</div>
                  ))}
                  <div style={{ display: 'flex', whiteSpace: 'pre' }}>
                    <span style={{ color: 'var(--accent)' }}>{this.promptOf(conNode)}</span>
                    <span>{s.conInput}</span>
                    <span style={{ display: 'inline-block', width: 7, height: 13, background: '#cde5d8', animation: 'blinkCursor 1.1s step-end infinite', marginLeft: 1 }} />
                  </div>
                  <input
                    ref={(el) => { this.conInputEl = el; }}
                    value={s.conInput}
                    onChange={(e) => this.setState({ conInput: e.target.value })}
                    onKeyDown={(e) => {
                      const c = conC; if (!c) return;
                      if (e.key === 'Enter') { const v = s.conInput; this.setState({ conInput: '' }); this.runCmd(v); this.forceUpdate(); }
                      else if (e.key === 'ArrowUp') { e.preventDefault(); if (c.hi > 0) { c.hi--; this.setState({ conInput: c.hist[c.hi] || '' }); } }
                      else if (e.key === 'ArrowDown') { e.preventDefault(); if (c.hi < c.hist.length) { c.hi++; this.setState({ conInput: c.hist[c.hi] || '' }); } }
                    }}
                    style={{ position: 'absolute', opacity: 0, pointerEvents: 'none', left: -999 }}
                  />
                </div>
              </>
            ) : (
              <div style={{ flex: 1, display: 'grid', placeItems: 'center', color: 'var(--muted)', fontSize: 12.5, textAlign: 'center', padding: 30 }}>No session — click a device on the canvas to open a real CLI over SSH.</div>
            ))}

            {/* configs tab */}
            {s.tab === 'configs' && (configKeys.length > 0 ? (
              <>
                <div style={{ display: 'flex', gap: 6, padding: '10px 12px', flexWrap: 'wrap', flex: '0 0 auto' }}>
                  {configKeys.map((k) => {
                    const nd = s.nodes.find(n => n.id === k);
                    return (
                      <button
                        key={k}
                        onClick={() => this.setState({ configSel: k })}
                        style={{
                          border: `1px solid ${cfgSel === k ? 'var(--accent)' : 'var(--border)'}`,
                          background: cfgSel === k ? 'var(--accentSoft)' : 'var(--panel2)',
                          color: cfgSel === k ? 'var(--accent)' : 'var(--muted)',
                          borderRadius: 7, padding: '4px 9px', fontSize: 11, fontWeight: 600, fontFamily: mono, cursor: 'pointer',
                        }}
                      >
                        {nd ? nd.name : k}
                      </button>
                    );
                  })}
                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '0 14px 8px', flex: '0 0 auto' }}>
                  <span style={{ fontSize: 11, color: 'var(--muted)', fontFamily: mono }}>{cfg && cfgNode ? V[cfgNode.vendor].file(cfgNode.name) : (cfg ? 'config' : '')}</span>
                  <span style={{ flex: 1 }} />
                  {!!(cfg && cfg.diff) && <span style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--amber)', background: 'rgba(232,179,72,.12)', borderRadius: 5, padding: '2px 7px' }}>modified by agent</span>}
                  <span style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--green)', background: 'rgba(62,207,142,.1)', borderRadius: 5, padding: '2px 7px' }}>deployed</span>
                </div>
                <div style={{ flex: 1, overflow: 'auto', background: '#0a0c0f', borderTop: '1px solid var(--border)', padding: '10px 0', fontFamily: mono, fontSize: 11.5, lineHeight: 1.6, userSelect: 'text' }}>
                  {(cfg ? cfg.lines : []).map((l, i) => (
                    <div key={i} style={{ display: 'flex', background: l.s === 'add' ? 'rgba(62,207,142,.08)' : l.s === 'del' ? 'rgba(240,101,90,.08)' : 'transparent' }}>
                      <span style={{ width: 34, flex: '0 0 34px', textAlign: 'right', paddingRight: 10, color: '#3d4756' }}>{i + 1}</span>
                      <span style={{ width: 14, flex: '0 0 14px', color: l.s === 'add' ? 'var(--green)' : 'var(--red)' }}>{l.s === 'add' ? '+' : l.s === 'del' ? '−' : ''}</span>
                      <span style={{ whiteSpace: 'pre', color: l.s === 'del' ? '#7b8494' : '#cdd6e4' }}>{l.t}</span>
                    </div>
                  ))}
                </div>
              </>
            ) : (
              <div style={{ flex: 1, display: 'grid', placeItems: 'center', color: 'var(--muted)', fontSize: 12.5, textAlign: 'center', padding: 30 }}>No configs yet — the agent writes per-vendor configurations when it designs a lab.</div>
            ))}

            {/* validation tab */}
            {s.tab === 'validation' && ((s.checks.length > 0 || !!vs) ? (
              <>
                <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '14px 16px 10px', flex: '0 0 auto' }}>
                  <div style={{ width: 38, height: 38, borderRadius: 11, background: vs && vs.warn ? 'rgba(232,179,72,.13)' : 'rgba(62,207,142,.12)', display: 'grid', placeItems: 'center' }}>
                    <svg width={19} height={19} viewBox="0 0 24 24" fill="none" stroke={vs && vs.warn ? 'var(--amber)' : 'var(--green)'} strokeWidth={2} strokeLinecap="round" strokeLinejoin="round"><path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z" /></svg>
                  </div>
                  <div style={{ flex: 1 }}>
                    <div style={{ fontSize: 13.5, fontWeight: 600 }}>{vs ? (vs.warn ? `${vs.passed}/${vs.total} passed · ${vs.warn} warning` : `All checks passing (${vs.passed}/${vs.total})`) : 'Validating…'}</div>
                    <div style={{ fontSize: 11.5, color: 'var(--muted)', marginTop: 1 }}>{vs ? 'Last run ' + vs.when + ' · on real device CLIs' : 'Checks are streaming in'}</div>
                  </div>
                  <button
                    className="btn-ghost"
                    onClick={() => { if (!s.running && s.deployed) this.scnRevalidate(); }}
                    disabled={s.running}
                    style={{ border: '1px solid var(--border2)', background: 'var(--panel2)', color: 'var(--text)', borderRadius: 8, padding: '6px 11px', fontSize: 11.5, fontWeight: 600, cursor: 'pointer' }}
                  >
                    Re-run
                  </button>
                </div>
                <div style={{ flex: 1, overflowY: 'auto', padding: '4px 14px 16px', display: 'flex', flexDirection: 'column', gap: 7 }}>
                  {s.checks.map((c, ci) => (
                    <div key={ci} style={{ background: 'var(--panel2)', border: '1px solid var(--border)', borderRadius: 10, padding: '10px 12px', animation: 'fadeUp .25s ease' }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                        <span style={{ width: 16, height: 16, flex: '0 0 16px', borderRadius: '50%', background: c.status === 'pass' ? 'rgba(62,207,142,.15)' : c.status === 'warn' ? 'rgba(232,179,72,.15)' : 'rgba(240,101,90,.15)', display: 'grid', placeItems: 'center' }}>
                          <svg width={9} height={9} viewBox="0 0 24 24" fill="none" stroke={c.status === 'pass' ? 'var(--green)' : c.status === 'warn' ? 'var(--amber)' : 'var(--red)'} strokeWidth={3.2} strokeLinecap="round" strokeLinejoin="round">
                            <path d={c.status === 'pass' ? 'M4 12.5 9.5 18 20 6.5' : c.status === 'warn' ? 'M12 5v9M12 18.5v.01' : 'M5 5l14 14M19 5 5 19'} />
                          </svg>
                        </span>
                        <span style={{ fontSize: 12.5, fontWeight: 600 }}>{c.label}</span>
                        <span style={{ marginLeft: 'auto', fontSize: 10.5, fontWeight: 700, color: c.status === 'pass' ? 'var(--green)' : c.status === 'warn' ? 'var(--amber)' : 'var(--red)', letterSpacing: '.5px' }}>
                          {c.status === 'pass' ? 'PASS' : c.status === 'warn' ? 'WARN' : 'FAIL'}
                        </span>
                      </div>
                      <div style={{ fontSize: 11.5, color: 'var(--muted)', marginTop: 5, marginLeft: 24, fontFamily: mono, userSelect: 'text' }}>{c.detail}</div>
                    </div>
                  ))}
                </div>
              </>
            ) : (
              <div style={{ flex: 1, display: 'grid', placeItems: 'center', color: 'var(--muted)', fontSize: 12.5, textAlign: 'center', padding: 30 }}>No validation run yet — the agent validates every lab after deployment.</div>
            ))}

            {/* sessions tab */}
            {s.tab === 'sessions' && (s.sessions.length > 0 ? (
              <>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '12px 14px 8px', flex: '0 0 auto' }}>
                  <span style={{ fontSize: 11, color: 'var(--muted)' }}>Everything in this lab session — prompts, agent actions, SSH activity.</span>
                  <span style={{ flex: 1 }} />
                  <button className="btn-ghost" onClick={exportSessions} style={{ border: '1px solid var(--border2)', background: 'var(--panel2)', color: 'var(--text)', borderRadius: 8, padding: '5px 10px', fontSize: 11, fontWeight: 600, cursor: 'pointer', whiteSpace: 'nowrap' }}>Export .log</button>
                  <button className="btn-danger" onClick={() => this.setState({ sessions: [] })} style={{ border: '1px solid var(--border)', background: 'transparent', color: 'var(--muted)', borderRadius: 8, padding: '5px 10px', fontSize: 11, fontWeight: 600, cursor: 'pointer', whiteSpace: 'nowrap' }}>Clear</button>
                </div>
                <div ref={(el) => { this.sessEl = el; }} style={{ flex: 1, overflowY: 'auto', padding: '4px 12px 14px', display: 'flex', flexDirection: 'column', gap: 5 }}>
                  {s.sessions.map((e, ei) => {
                    const K = sessionKindStyle(e.kind);
                    return (
                      <div key={ei} style={{ display: 'flex', gap: 9, alignItems: 'flex-start', background: 'var(--panel2)', border: '1px solid var(--border)', borderRadius: 9, padding: '8px 10px', animation: 'fadeUp .2s ease' }}>
                        <span style={{ fontSize: 10, color: 'var(--muted)', fontFamily: mono, marginTop: 3, whiteSpace: 'nowrap' }}>{new Date(e.ts).toTimeString().slice(0, 8)}</span>
                        <span style={{ fontSize: 9, fontWeight: 700, letterSpacing: '.5px', color: K.fg, background: K.bg, borderRadius: 4, padding: '2px 6px', marginTop: 1, whiteSpace: 'nowrap' }}>{K.kind}</span>
                        <span style={{ fontSize: 11.5, lineHeight: 1.5, color: 'var(--text)', userSelect: 'text', minWidth: 0, overflowWrap: 'anywhere' }}>{e.text}</span>
                      </div>
                    );
                  })}
                </div>
              </>
            ) : (
              <div style={{ flex: 1, display: 'grid', placeItems: 'center', color: 'var(--muted)', fontSize: 12.5, textAlign: 'center', padding: 30 }}>Nothing recorded yet — every prompt, agent action, and SSH command lands here.</div>
            ))}
          </div>
        </div>
      </div>
    );
  }
}
