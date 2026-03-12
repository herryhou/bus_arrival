require('dotenv').config();
const fetch = require('node-fetch');
const fs = require('fs');

const CLIENT_ID = process.env.TDX_CLIENT_ID;
const CLIENT_SECRET = process.env.TDX_CLIENT_SECRET;

async function getAccessToken() {
  const tokenUrl = 'https://tdx.transportdata.tw/auth/realms/TDXConnect/protocol/openid-connect/token';
  const data = new URLSearchParams({
    grant_type: 'client_credentials',
    client_id: CLIENT_ID,
    client_secret: CLIENT_SECRET
  });

  try {
    const response = await fetch(tokenUrl, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/x-www-form-urlencoded',
      },
      body: data
    });

    // ✅ 完整錯誤處理：先檢查狀態碼
    if (!response.ok) {
      const errorText = await response.text();
      console.error('❌ 認證失敗:', response.status, errorText);
      throw new Error(`HTTP ${response.status}: ${errorText}`);
    }

    const dataJson = await response.json();
    console.log('✅ Token 取得成功，過期時間:', dataJson.expires_in, '秒');
    return dataJson.access_token;

  } catch (error) {
    console.error('❌ 認證請求錯誤:', error.message);
    throw error;
  }
}

async function downloadGTFS(city = 'Taipei') {
  try {
    const token = await getAccessToken();

    const apiUrl = `https://ptx.transportdata.tw/MOTC/v3/GTFS/Bus/City/${city}?$format=ZIP`;

    console.log('📡 下載 GTFS:', apiUrl);

    const response = await fetch(apiUrl, {
      headers: { 'Authorization': `Bearer ${token}` },
    });

    if (!response.ok) {
      const errorText = await response.text();
      console.error('❌ GTFS 下載失敗:', response.status, errorText);
      return;
    }

    const buffer = await response.buffer();
    const filename = `gtfs_${city.toLowerCase()}.zip`;
    fs.writeFileSync(filename, buffer);

    console.log(`✅ GTFS 下載完成: ${filename}`);
    console.log(`📊 檔案大小: ${(buffer.length/1024/1024).toFixed(2)} MB`);

  } catch (error) {
    console.error('❌ 總錯誤:', error.message);
  }
}

// 執行
downloadGTFS('Taipei');
