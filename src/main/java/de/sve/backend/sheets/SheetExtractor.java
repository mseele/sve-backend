package de.sve.backend.sheets;

import java.io.IOException;
import java.security.GeneralSecurityException;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import com.google.api.services.sheets.v4.model.ValueRange;

public class SheetExtractor extends AbstractSheetController {

	public SheetExtractor(String spreadsheetId) throws GeneralSecurityException, IOException {
		super(spreadsheetId);
	}

	public List<Map<String, String>> get(int gid, List<String> keys) throws IOException {
		List<Map<String, String>> result = new ArrayList<>();
		String sheetTitle = getSheetTitle(gid);
		String range = "'" + sheetTitle + "'!A1:Z1000"; //$NON-NLS-1$ //$NON-NLS-2$
		ValueRange response = this.fSheets.spreadsheets().values().get(this.fSpreadsheetId, range).execute();
		List<List<Object>> values = response.getValues();
		if (values != null) {
			Map<String, Integer> keyMapping = new HashMap<>();
			int minRowSize = 0;
			for (List<Object> row : values) {
				if (row == null || row.isEmpty() || row.get(0) == null) {
					break;
				}
				if (keyMapping.isEmpty()) {
					for (int i = 0; i < row.size(); i++) {
						String value = String.valueOf(row.get(i));
						if (keys.contains(value)) {
							keyMapping.put(value, i);
						}
						if (keyMapping.size() == keys.size()) {
							minRowSize = i + 1;
							break;
						}
					}
				} else if (row.size() >= minRowSize) {
					Map<String, String> entry = new HashMap<>();
					keyMapping.forEach((key, index) -> {
						entry.put(key, String.valueOf(row.get(index)));
					});
					result.add(entry);
				}
			}
			if (keyMapping.size() != keys.size()) {
				throw new IOException("Not all keys (" + keys + ") have been found in the first row (" + values.get(0) + ")"); //$NON-NLS-1$ //$NON-NLS-2$ //$NON-NLS-3$
			}
		}
		return result;
	}

}
