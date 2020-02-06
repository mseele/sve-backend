package de.sve.backend.sheets;

import java.io.IOException;
import java.security.GeneralSecurityException;
import java.text.NumberFormat;
import java.time.LocalDateTime;
import java.time.ZoneId;
import java.time.format.DateTimeFormatter;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.stream.Collectors;

import org.apache.commons.text.StringEscapeUtils;

import com.google.api.client.auth.oauth2.Credential;
import com.google.api.client.googleapis.javanet.GoogleNetHttpTransport;
import com.google.api.client.json.jackson2.JacksonFactory;
import com.google.api.services.sheets.v4.Sheets;
import com.google.api.services.sheets.v4.Sheets.Spreadsheets.Values.Update;
import com.google.api.services.sheets.v4.model.UpdateValuesResponse;
import com.google.api.services.sheets.v4.model.ValueRange;

import de.sve.backend.model.events.Event;
import de.sve.backend.model.events.EventBooking;

public class EventsSheetController extends AbstractSheetController {

	private static DateTimeFormatter DATE_TIME_FORMAT = DateTimeFormatter.ofPattern("dd.MM.yyyy HH:mm:ss"); //$NON-NLS-1$

	private static NumberFormat PRICE_FORMAT = NumberFormat.getCurrencyInstance(Locale.GERMANY);

	public static String saveBooking(EventBooking booking, Event event) throws GeneralSecurityException, IOException {
		Credential credential = authorize();
		Sheets sheets = new Sheets.Builder(GoogleNetHttpTransport.newTrustedTransport(), JacksonFactory.getDefaultInstance(), credential).build();
		
		String spreadsheetId = event.sheetId();
		Integer sheetId = Integer.valueOf(event.gid().intValue());
		
		String sheetTitle = getSheetTitle(sheets, spreadsheetId, sheetId);
		int rowIndex = getRowIndex(sheets, spreadsheetId, sheetTitle);
		return insert(sheets, spreadsheetId, sheetTitle, rowIndex, booking, event);
	}

	private static int getRowIndex(Sheets sheets, String spreadsheetId, String sheetTitle) throws IOException {
		String range = "'" + sheetTitle + "'!B2:B100"; //$NON-NLS-1$ //$NON-NLS-2$
		ValueRange response = sheets.spreadsheets().values().get(spreadsheetId, range).execute();
		int index = 2;
		List<List<Object>> values = response.getValues();
		if (values != null) {
			for (List<Object> row : values) {
				if (row.get(0) == null) {
					break;
				}
				index++;
			}
		}
		return index;
	}

	private static String insert(Sheets sheets, String spreadsheetId, String sheetTitle, int rowIndex, EventBooking booking, Event event) throws IOException {
		String range = "'" + sheetTitle + "'!B" + rowIndex + ":L" + rowIndex; //$NON-NLS-1$ //$NON-NLS-2$ //$NON-NLS-3$

		List<Object> content = Arrays.asList(
    		DATE_TIME_FORMAT.format(LocalDateTime.now(ZoneId.of("Europe/Berlin"))), //$NON-NLS-1$
        	booking.firstName(),
        	booking.lastName(),
        	booking.street(),
        	booking.city(),
        	booking.email(),
        	booking.phone() != null ? "'" + booking.phone() : "", //$NON-NLS-1$ //$NON-NLS-2$
        	booking.isMember() ? "J" : "N", //$NON-NLS-1$ //$NON-NLS-2$
        	PRICE_FORMAT.format(booking.cost(event)),
        	"N", //$NON-NLS-1$
        	booking.comments()
        ); 
		List<List<Object>> values = Arrays.asList(content);
		ValueRange body = new ValueRange().setValues(values);
		Update request = sheets.spreadsheets().values().update(spreadsheetId, range, body);
		request.setValueInputOption("USER_ENTERED"); //$NON-NLS-1$
		UpdateValuesResponse result = request.execute();
		return result.getUpdatedCells() +
			   " cells updated:</br></br>" + //$NON-NLS-1$
			   content.stream()
					  .map(o -> StringEscapeUtils.escapeHtml4(String.valueOf(o)))
					  .collect(Collectors.joining("</br>- ", "- ", "")); //$NON-NLS-1$ //$NON-NLS-2$ //$NON-NLS-3$
	}

}
