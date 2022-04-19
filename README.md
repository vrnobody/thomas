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
根目录有client.json, server.json两个配置样例。其中client.json的`listen`填写本地SOCKS5监听地址。`length`设置代理链长度（即跳转次数）。`outlets`设置出口服务器。`relays`设置中继服务器。客户端创建代理链时会从`outlets`中随机抽取1个节点作为最终出口，然后随机抽取 length - 1 个`relays`节点作为中继。代理数据依次经过各节点最后到达目标网站。当`length`设置为1时，客户端将只从`outlets`中抽取1个节点，不抽取中继节点。  

#### 注意事项
服务端如果放公网需要前置Nginx/Caddy做TLS终结，相应client.json的`addr`配置项由`ws://...`改为`wss://...`。这个软件没对数据流做任何加密，所以不要作死直接放公网。  

#### 编译
[Release](https://github.com/vrnobody/thomas/releases/latest)页面有编译好的Windows、Linux、ARM等二进制文件。其他架构需自行安装Rust，然后输入`cargo build --release`进行编译。最后生成的程序文件位于`target/release`目录内。  
