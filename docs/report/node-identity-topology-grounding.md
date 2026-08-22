# 노드 정체성·통신 규약의 외부 그라운딩 — 싱글/멀티 토폴로지 축 정립

> 문서형: report (docs/report 무일자 예외 — kebab slug)
> 작성: 2026-08-22 KST · grounded-citations ledger + evidence 게이트 기반
> 목적: 사용자의 롤모델 가설(멀티=분산처리·버전싱크 / 싱글=오케스트레이터+자율 sub+HITL 조율)을
> NCCL·Ray·A2A·ROS2·CAN·SPI의 권위 소스로 **적대적 검증**하고, `nodes:[main,sub]` 단일 스킴이
> 양 토폴로지에서 서로 다른 "sub" 의미를 흐리는 문제의 개선 방향을 도출한다.

## 서론 — 왜 "sub" 하나로 두 세계를 덮는가

`output/single/manifest.yaml`과 `output/multi/manifest.yaml`은 동일한 `nodes[].role: main|sub`
스킴을 쓴다. 그러나 두 통로에서 "sub"가 가리키는 존재는 본질적으로 다르다. 이 문서는 그 차이를
외부 정본으로 고정하고, 하네스가 토폴로지를 제1축으로 삼지 않은 것이 문제의 뿌리임을 입증한다.

## 1. 분산 컴퓨팅의 정체성 — NCCL · Ray (멀티)

NCCL은 커뮤니케이터를 만들 때 "n개의 CUDA 디바이스 각각에 0과 n-1 사이의 고유한 rank를 할당해야
한다"고 규정한다.[1] 이 rank는 **위치기반(positional)** 식별자이며, 커뮤니케이터 소속은
`ncclGetUniqueId()`가 반환하는 ID를 "모든 참여 스레드·프로세스에 브로드캐스트"해 공유한다.[1]
즉 NCCL의 정체성 = **위치(rank) + 공유 비밀(uniqueId)** 두 축이다. sub↔sub 통신은 allreduce 같은
**집단 연산(collective)**으로, 이는 대칭적 peer 교환이지 주소기반 point-to-point가 아니다.[unverified]

Ray 클러스터는 "단일 head 노드와 임의 개수의 연결된 worker 노드로 구성"되며, head가 "추가 제어
프로세스"를 실행하고 worker 수는 "애플리케이션 수요에 따라 autoscale"된다.[2] 이는 사용자가 말한
"1 메인 + N-1 서브" 구조의 정확한 외부 정본이다. 여기서 서브는 **제어 평면은 head에 종속**되고
**데이터 평면은 peer 간 집단 통신**을 하는 워커다.

## 2. 에이전트 오케스트레이션의 정체성 — A2A (싱글)

A2A 프로토콜은 "독립적이며 잠재적으로 불투명한(opaque) AI 에이전트 시스템 간의 통신·상호운용"을
위한 개방 표준이며, 에이전트들이 "서로의 내부 상태·메모리·도구에 접근할 필요 없이" 협업하게
한다.[3] 이는 사용자의 싱글노드 모델("각 서브는 할당 업무를 자율 처리")과 정확히 같은 설계다.

A2A의 정체성·주소지정은 세 개념으로 요약된다:

- **A2A Client** — "사용자 또는 다른 시스템을 대신해 A2A Server에 요청을 개시하는 애플리케이션/에이전트".[3]
- **Agent Card** — "A2A Server가 발행하는 JSON 메타데이터 문서로, 그 **정체성·능력·스킬·서비스
  엔드포인트·인증 요구사항**을 기술한다".[3] → 워크스페이스의 `Agent_Card.json`이 차용한 원본이다.
- **Task** — "고유 ID로 식별되는 작업의 기본 단위. stateful하며 정의된 라이프사이클을 진행한다".[3]

여기서 핵심은 **정체성 = 능력 카드(AgentCard) + 서비스 엔드포인트**이고, **주소지정 = 클라이언트가
서버(원격 에이전트)를 호출**하는 client-server 모델이라는 점이다. sub↔sub 직접 통신은 A2A 코어
모델에 없다 — 각 sub는 메인(클라이언트)과만 대화하는 독립 에이전트다.

## 3. 로보틱스 미들웨어의 정체성 — ROS2 ("sub-a vs sub-b를 어떻게 식별?") 

ROS2는 "node가 ROS 2 그래프의 참여자"이며 "각 node는 하나의 논리적 일을 해야 한다"고 정의하고,
"node 간 연결은 분산 디스커버리 과정을 통해 수립된다"고 명시한다.[4] 디스커버리는 "미들웨어가
자동으로" 수행하며, node는 "같은 ROS 도메인(ROS_DOMAIN_ID 환경변수로 설정)의 다른 node에게 자신의
존재를 광고"한다.[5]

식별·명명 측면에서 ROS2가 주는 답은 세 가지다:

- **고유 이름**: 실행파일 이름이 그래프상 node 이름과 같지 않을 수 있으며, `__node:=my_turtle`
  remapping으로 node 이름을 재지정한다.[6]
- **네임스페이스**: `/turtlesim`, `/my_turtle`처럼 슬래시가 계층(네임스페이스)을 표현한다.[6]
- **도메인 스코핑**: `ROS_DOMAIN_ID`로 어느 node끼리 서로를 발견할지를 가른다.[5]

이는 사용자 질문 "sub-a와 sub-b를 어떻게 식별하나"에 대한 **직접적 외부 정본**이다: 위치(rank)도,
우선순위(메시지 ID)도 아닌 **고유 이름 + 네임스페이스 + 도메인 스코프**로 식별한다.

## 4. 버스 주소지정의 두 극단 — CAN · SPI (차용 판단용)

CAN은 "broadcast 기반·메시지 지향 프로토콜로, arbitration이라는 과정을 통해 데이터 무결성과
우선순위를 보장한다"고 정의된다.[7] 메시지는 11비트(표준) 또는 29비트(확장) 식별자를 가지며,[7]
식별자가 곧 우선순위다. 즉 CAN은 **노드 주소가 없고, 메시지 ID가 우선순위를 결정**한다.

SPI는 대조적으로 "슬레이브는 고유 주소가 필요하지 않으며"(I²C·SCSI와 달리), "디바이스당 최대 하나의
고유 신호(SS, chip-select); 나머지는 모두 공유"한다.[8] 즉 SPI는 **마스터가 전용 선택선(SS)으로
슬레이브를 명시적으로 지정**하고, 단일 마스터라 arbitration이 없다.[8]

이 두 극단은 하네스가 어느 쪽도 따르지 말아야 함을 보여준다. 멀티(Ray/NCCL)는 **집단 연산의
대칭 peer**이고, 싱글(A2A)은 **클라이언트-서버**다. CAN의 메시지-우선순위도, SPI의 하드웨어
선택선도 에이전트 하네스엔 맞지 않는다 — 필요한 것은 **역할·정체성의 선언(AgentCard/A2A)과
위치·집단의 선언(NCCL/Ray)을 토폴로지별로 분리**하는 것이다.

## 5. 정체성 모델 종합

| 출처 | 정체성 식별 | 주소지정 | sub↔sub 통신 | 적용 |
|---|---|---|---|---|
| NCCL | rank(위치) + uniqueId(공유 비밀) | 집단 연산(커뮤니케이터) | 대칭 collective | 멀티 데이터평면 |
| Ray | head(1) + worker(N-1) | GCS/스케줄러 경유 | peer 집단 | 멀티 |
| A2A | AgentCard(정체성+능력+엔드포인트) | client→server 호출 | 없음(각자 독립) | 싱글 |
| ROS2 | 고유 이름 + 네임스페이스 + 도메인 | 자동 디스커버리(DDS) | pub/sub | (명명·스코핑 차용) |
| CAN | 메시지 ID = 우선순위 | broadcast arbitration | broadcast | (부적합 — 반례) |
| SPI | 전용 SS 선택선 | 마스터 명시 선택 | 없음 | (부적합 — 반례) |

## 6. 사용자 롤모델의 적대적 검증

### 6.1 "멀티 = 버전/드라이버 싱크가 분산처리 목표 달성의 조건"

**판정: 실질 정합, 단 메커니즘은 정정 필요.** 버전 싱크가 필요한 이유의 1차 근거는 "연산이 한쪽으로
쏠리는 것 방지"(로드 불균형)가 아니라 **집단 연산의 lockstep 정합성**이다. NCCL의 rank-커뮤니케이터
모델은 모든 rank가 동일한 집단 연산에 동시 참여하는 장벽(barrier) 구조이므로,[1][unverified] 참여 노드의
바이너리·드라이버가 어긋나면 교착 또는 무결성 손상이 된다.[unverified] 로드 쏠림(straggler)은 이 장벽의
**2차 효과**다.[unverified] 따라서 하네스의 불변식 표현도 "싱크 = 로드균형"이 아니라 "싱크 = 집단 연산 ABI 정합"
으로 옮겨야 정확하다. 동일 버전 필수라는 정확한 문장은 NCCL 문서에서 직접 확인하지 못했으며,
집단 연산 의미론에서 파생된다.[unverified]

### 6.2 "싱글 = 메인이 오케스트레이터, 서브가 자율 처리, HITL은 메인이 조율"

**판정: 정합.** A2A의 client-server 모델이 정확히 이 구조다 — client(메인)가 task를 개시하고
server(서브=원격 에이전트)가 자율 처리하며,[3] 에이전트는 불투명해 내부 상태를 공유하지 않고,[3]
task는 고유 ID로 라이프사이클을 관리한다.[3] "서브의 HITL 질문을 메인이 조율"한다는 것은 client가
task 라이프사이클(상태 전이·재협상)을 관리하고 사용자 입력을 중계한다는 A2A 의미론과 일치한다.
다만 A2A 코어 사양은 "오케스트레이터"라는 계급을 명시하지 않으며, 오케스트레이션은 client가 여러
server를 호출하는 **패턴**으로 성립한다는 점을 주의해야 한다.[3][unverified]

## 7. 결론 — 하네스 개선 방향 (증거그래프·안전불변식 연결)

외부 정본이 지지하는 개선 축은 하나로 수렴한다: **토폴로지가 노드 제어의 제1축이어야 한다.** 그리고
"sub"는 두 개의 서로 다른 정체성 계약으로 분리되어야 한다.

1. **정체성 축 분리**: 멀티 sub = Ray 워커(rank/집단 연산, 메인과 ABI 동기), 싱글 sub = A2A
   원격 에이전트(AgentCard 정체성, 자율 처리, client-server). 한 스킴의 `role:sub`로 묶는 것이
   혼동의 직접 원인이다.
2. **노드 정체성 role 스킴(§2.7.6, "미실장")의 외부 정본 확립**: NCCL/Ray의 "위치+집단"과 A2A의
   "능력카드+엔드포인트", ROS2의 "고유이름+네임스페이스+도메인" 중 하네스는 **A2A 능력카드(기존
   Agent_Card.json)를 싱글 축의 정체성 권위로, manifest `nodes[].role`을 멀티 축의 위치 권위로**
   분리 정립할 수 있다. ROS2의 도메인 스코핑은 "어느 sub가 어느 업무를 받을 수 있는가"의 자율성
   범위 차등 설정에 차용 가능하다.
3. **증거그래프·안전불변식으로의 연결**: 위 1·2는 "어느 토폴로지의 어느 정책·스킬이 어느 sub
   의미를 인용·검증하는가"가 명시되지 않은 **누락 커버리지** 결함이다. 증거그래프는 이 의무
   누락을 하드게이트로 강제하고, 안전불변식은 "싱글 sub는 자율/멀티 sub는 동기"라는 항상-참
   원칙(왜)을 고정한다 — 이 축은 후속 인터뷰에서 확정한다.

## Sources

[1] https://docs.nvidia.com/deeplearning/nccl/user-guide/docs/usage/communicators.html — NCCL: Creating a Communicator (NVIDIA Docs)
    > "When creating a communicator, a unique rank between 0 and n-1 has to be assigned to each of the n CUDA devices which are part of the communicator."
    > "Before calling ncclCommInitRank() , you need to first create a unique object which will be used by all processes and threads to synchronize and understand they are part of the same communicator. This is done by calling the ncclGetUniqueId() function."
    > "The ncclGetUniqueId() function returns an ID which has to be broadcast to all participating threads and processes using any CPU communication system"
[2] https://docs.ray.io/en/latest/cluster/key-concepts.html — Ray Cluster Key Concepts (Ray Docs)
    > "A Ray cluster consists of a single head node and any number of connected worker nodes"
    > "The head node runs additional control processes"
    > "The number of worker nodes may be autoscaled with application demand"
[3] https://a2a-protocol.org/latest/specification — A2A Protocol Specification (a2a-protocol.org)
    > "The Agent2Agent (A2A) Protocol is an open standard designed to facilitate communication and interoperability between independent, potentially opaque AI agent systems."
    > "Securely exchange information to achieve user goals without needing access to each other's internal state, memory, or tools."
    > "Agent Card: A JSON metadata document published by an A2A Server, describing its identity, capabilities, skills, service endpoint, and authentication requirements."
    > "A2A Client: An application or agent that initiates requests to an A2A Server on behalf of a user or another system."
    > "Task: The fundamental unit of work managed by A2A, identified by a unique ID. Tasks are stateful and progress through a defined lifecycle."
[4] https://github.com/ros2/ros2_documentation/blob/rolling/source/ROS-Framework/About-Nodes.rst — ROS 2: About Nodes (ros2_documentation)
    > "A node is a participant in the ROS 2 graph"
    > "Nodes are typically the unit of computation in a ROS graph; each node should do one logical thing."
    > "Nodes can communicate with other nodes within the same process, in a different process, or on a different machine."
[5] https://github.com/ros2/ros2_documentation/blob/rolling/source/ROS-Framework/nodes/About-Discovery.rst — ROS 2: About Discovery (ros2_documentation)
    > "Discovery of nodes happens automatically through the underlying middleware of ROS 2."
    > "When a node is started, it advertises its presence to other nodes on the network with the same ROS domain (set with the ROS_DOMAIN_ID environment variable)."
[6] https://github.com/ros2/ros2_documentation/blob/rolling/source/ROS-Framework/nodes/Working-with-nodes/Understanding-ROS2-Nodes/Understanding-ROS2-Nodes.rst — ROS 2: Understanding Nodes tutorial
    > "The executable name is not always the same as the node name on the ROS graph."
    > "You can also remap the node name when you start a new node."
[7] https://en.wikipedia.org/wiki/CAN_bus — CAN bus (Wikipedia)
    > "This broadcast-based , message-oriented protocol ensures data integrity and prioritization through a process called arbitration , allowing the highest priority device to continue transmitting if multiple devices attempt to send data simultaneously, while others back off."
    > "Part A is for the standard format with an 11-bit identifier, and part B is for the extended format with a 29-bit identifier."
[8] https://en.wikipedia.org/wiki/Serial_Peripheral_Interface — Serial Peripheral Interface (Wikipedia)
    > "Slaves do not need a unique address"
    > "At most one unique signal per device ( SS ); all others are shared"
