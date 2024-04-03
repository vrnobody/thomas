### Thomas  
这是一个websocket代理链小工具，用于将多个ws服务器串连成一条动态的代理链。    
[![Total Downloads][1]][2]  

[1]: https://img.shields.io/github/downloads/vrnobody/thomas/total.svg "Total Downloads Badge"
[2]: https://somsubhra.github.io/github-release-stats/?username=vrnobody&repository=thomas&per_page=30 "Download Details"

#### 用法
```bash
# 服务器
server -c server.json

# 客户端
client -c client.json
```

#### 配置文件
根目录有[client.json](https://github.com/vrnobody/thomas/blob/main/client.json), [server.json](https://github.com/vrnobody/thomas/blob/main/server.json)两个配置样例。  

server.json说明
```jsonc
{
    // 实际使用时不可以有注释！！
    "loglevel": "info", // debug, info, wran, error
    "listen": "127.0.0.1:3001",  // ws协议监听的IP和端口
    "pubkey": "cyBvyuctYPhWQmKQgHLT9tvoTMt2ujt3115UzehBhX4=",  // 通过 server --key 生成，可以公布
    "secret": "v16H1K1N/zP+WU4MxlLY9/RcdOSKKC8pcMpJchHIqBw="  // 不可以公布，注意保密
}
```

client.json说明
```jsonc
{
    "loglevel": "info", // 同server.json
    "listen": "127.0.0.1:1080", // 支持http和socks5两种协议，不支持账号密码验证，不支持https
    "length": 2,  // 随机挑选多少个relays节点
    "proxy": "http://127.0.0.1:8080",  // 前置代理，支持http和socks5两种协议，可以留空但不可以省略
    "inlets": [
        {
            "name": "In1",  // 随便给个名字
            "addr": "ws://127.0.0.1:3001",  // 服务器地址，前置TLS的改成wss://...
            "pubkey": "cyBvyuctYPhWQmKQgHLT9tvoTMt2ujt3115UzehBhX4="  // 上面server.json中的pubkey
        },
        { ... },
        ...
    ],
    "outlets": [], // 和inlets相同
    "relays": [] // 和inlets相同
}
```

#### 原理
客户端从listen接收到代理请求时，分别从inlets outlets抽1个节点，然后从relays中抽取length个节点，数据依顺经过inlet -> relay(s) -> outlet，最后到达目标地址。inlets relays outlets可以部分留空，节点总数大于等于1就行。  

#### 安全提醒
这个软件没对数据流做任何加密！！安全是一个说不完的话题，就算加上TLS还是有办法绕过。所以这个软件放弃了加密，只实现一个简单的数据管道，请配合其他代理软件一起使用以提高安全性。  

#### 编译
[Release](https://github.com/vrnobody/thomas/releases/latest)页面有编译好的Windows、Linux、ARM等二进制文件。其他架构需自行安装Rust，执行`cargo build --release`进行编译。最后生成的可执行文件位于`target/release`目录内。系统需要安装openssl v1.*。  
