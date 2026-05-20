---
name: Harness Engineering Direction
description: Zero_Nine follows Harness Engineering and Environment Engineering principles with "Agent Ecology" philosophy
type: project
---

## Zero_Nine Development Direction: Harness & Environment Engineering

**Decision**: Zero_Nine's evolution must follow Harness Engineering and Environment Engineering principles with an "Agent Ecology" mindset. All future development, features, and architectural decisions align with this direction.

**Why**: 
- Harness Engineering (驾驭工程) is the discipline of designing environments, constraints, and feedback loops that make AI agents reliable
- Environment Engineering (环境工程) focuses on building the world the agent operates in, not the agent itself
- **Agent Ecology Philosophy**: The future of AI agents requires an environment to support them — not just as technical components, but as:
  - A medium for behavior shaping (行为塑造的媒介)
  - Soil for evolutionary learning (演化学习的土壤)
  - Fundamental guarantee of intelligence reliability (智能可靠性的根本保障)
- **Gardener Mindset**: Good gardeners don't shape every leaf — they prepare soil, light, and water, letting plants grow naturally
- OpenAI's 2026 research identifies this as the key skill for AI development

**How to apply**:

### When designing new features:
1. Ask: "Does this add constraints/guardrails or observability?" (Harness)
2. Ask: "Does this improve the environment the agent operates in?" (Environment)
3. Ask: "Does this enable evolutionary learning?" (Ecology)
4. Avoid: Direct agent control mechanisms; prefer environmental shaping

### When evaluating architecture:
1. Does this increase steering capability without reducing agent autonomy?
2. Does this improve feedback signal quality for evolution?
3. Does this enhance recovery and replay capabilities?
4. Does this make the system more observable and traceable?
5. Does this create better "soil" for agent growth?

### Core Components Alignment:
- **zn-loop**: Scheduler + DAG constraints = Harness core
- **zn-exec**: Verification + evidence collection = Environment structure
- **zn-evolve**: Reward learning + curriculum + belief = Evolution ecosystem
- **zn-spec**: Specification artifacts = Structured context
- **zn-types**: Data models = Shared environment state
- **zn-host**: Plugin adapters = Environment boundaries

### Ecology Metaphor:
| Ecosystem | Zero_Nine | Agent Benefit |
|-----------|-----------|---------------|
| Soil (土壤) | Context, specs, skills | Foundation for grounding |
| Light (光照) | Rewards, feedback, confidence | Direction guidance |
| Water (水分) | Curriculum, belief, evolution | Nourishment for growth |
| Fences (围栏) | DAG, gates, verdicts | Prevent deviation |
| Pruning (修剪) | Retry, recovery, escalation | Correct deviation |

**References**:
- OpenAI: "Harness Engineering: leveraging Codex in an agent-first world"
- Industry consensus: "The rise of Agent Harness Engineering" (2025-2026)
- Zero_Nine: `docs/agent-philosophy.md` — Full philosophy document
