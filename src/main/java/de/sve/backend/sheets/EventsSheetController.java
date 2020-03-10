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

import com.google.api.services.sheets.v4.Sheets.Spreadsheets.Values.Update;
import com.google.api.services.sheets.v4.model.UpdateValuesResponse;
import com.google.api.services.sheets.v4.model.ValueRange;

import de.sve.backend.model.events.Event;
import de.sve.backend.model.events.EventBooking;

public class EventsSheetController extends AbstractSheetController {

	private static DateTimeFormatter DATE_TIME_FORMAT = DateTimeFormatter.ofPattern("dd.MM.yyyy HH:mm:ss"); //$NON-NLS-1$

	private static NumberFormat PRICE_FORMAT = NumberFormat.getInstance(Locale.GERMANY);

	public static String saveBooking(EventBooking booking, Event event) throws GeneralSecurityException, IOException {
		return new EventsSheetController(event.sheetId()).save(booking, event);
	}
	
	public EventsSheetController(String spreadsheetId) throws GeneralSecurityException, IOException {
		super(spreadsheetId);
	}

	private String save(EventBooking booking, Event event) throws IOException {
		Integer sheetId = Integer.valueOf(event.gid().intValue());

		String sheetTitle = getSheetTitle(sheetId);
		int rowIndex = getFirstEmptyRow(sheetTitle, "B", 2); //$NON-NLS-1$
		return insert(sheetTitle, rowIndex, booking, event);
	}

	private String insert(String sheetTitle, int rowIndex, EventBooking booking, Event event) throws IOException {
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
		Update request = this.fSheets.spreadsheets().values().update(this.fSpreadsheetId, range, body);
		request.setValueInputOption("USER_ENTERED"); //$NON-NLS-1$
		UpdateValuesResponse result = request.execute();
		return result.getUpdatedCells() +
			   " in " + range + " updated:" + //$NON-NLS-1$ //$NON-NLS-2$
			   content.stream()
					  .map(o -> StringEscapeUtils.escapeHtml4(String.valueOf(o)))
					  .collect(Collectors.joining(" | ")); //$NON-NLS-1$
	}

}
